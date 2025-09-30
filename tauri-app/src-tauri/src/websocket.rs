
//_______________________________________________________________
// src/websocket.rs
//
// Tauri v2 — WebSocket server with a fixed maximum number of concurrent connections.
// - Uses tokio + tokio-tungstenite
// - Limits active connections with a Semaphore (MAX_CONNECTIONS)
// - Sends a JSON "server busy" reply to excess clients and closes the connection
//
// Usage: call `start_websocket_server(app_handle.clone())` from your lib.rs setup block.

use std::sync::Arc; // Arc = atomically reference-counted pointer for sharing between tasks
use tauri::{AppHandle, Manager, Emitter}; // handle to the Tauri runtime / app (can be used to emit events later)
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use futures_util::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{Semaphore, OwnedSemaphorePermit};

///_______ Listening address/port_______________
pub const WS_PORT: u16 = 8080;
pub const WS_HOST: &str = "127.0.0.1";
pub const MAX_CONNECTIONS: usize = 1;

pub const DEBUG_WS: bool = true;


//_____________Struct _________________________

/// Generic request structure from client (CEP).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WsRequest {
    request_id: Option<u64>,     // optional; if present we echo it back in the reply so the client can match responses.
    command: String,
    payload: Value,
}

/// Generic reply structure sent back to clients
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WsResponse {
    request_id: Option<u64>,
    status: String,           // `status` is "ok" or "error".
    command: String,
    data: Value,             // holds the command result; 
}



//_____________fn __________________

/// Start the websocket server and keep it running in the background.
pub fn start_websocket_server(app_handle: AppHandle) {
    ///
    /// This function spawns a background async task (Tauri runtime) that:
    ///  - binds to WS_HOST:WS_PORT
    ///  - accepts incoming TCP connections
    ///  - upgrades them to WebSocket
    ///  - enforces MAX_CONNECTIONS using a Semaphore
    ///  - routes messages to `handle_command` and returns responses
    ///  Usage: Call `start_websocket_server(app.handle().clone())` from `lib.rs`'s setup.
    ///
    // Create a Semaphore with MAX_CONNECTIONS permits and wrap it in Arc so it can be shared.
    let sem = Arc::new(Semaphore::new(MAX_CONNECTIONS));

    // Spawn the server in Tauri's async runtime so it doesn't block the main thread.
    tauri::async_runtime::spawn(async move {
        // Bind a TCP listener to the configured host/port.
        let listener = TcpListener::bind((WS_HOST, WS_PORT))
            .await
            .expect("Failed to bind WebSocket listener");

        if DEBUG_WS {println!("🚀 WS server listening on ws://{}:{}", WS_HOST, WS_PORT);}

        // Accept loop: wait for incoming TCP connections forever.
        loop {
            // listener.accept() yields (TcpStream, SocketAddr)
            match listener.accept().await {
                Ok((stream, peer)) => {
                    // Clone handles to move into the spawned task
                    let sem = sem.clone();
                    let app_handle_clone = app_handle.clone();
                    let peer_str = peer.to_string();

                    // Spawn a task for each accepted TCP stream
                    tauri::async_runtime::spawn(async move {
                        // Step 1: perform the WebSocket handshake (upgrade)
                        match accept_async(stream).await {
                            Ok(ws_stream) => {
                                // Step 2: try to get a permit (non-blocking).
                                // If there's a permit, the client is accepted and handled.
                                // If no permit available, reply "server busy" and close connection.

                                match sem.try_acquire_owned() {
                                    Ok(permit) => {
                                        // We hold an OwnedSemaphorePermit (`permit`) for the
                                        // lifetime of this connection handler. When `permit` drops,
                                        // the semaphore count is released automatically.
                                        if let Err(e) = handle_connection(ws_stream, peer_str, app_handle_clone, permit).await {
                                            eprintln!("❌ Error handling client: {}", e);
                                        }
                                    }
                                    Err(_) => {
                                        // No permits available -> server is at full capacity.
                                        // Send a short JSON "server busy" message and close connection.
                                        if let Err(e) = reject_connection_busy(ws_stream, app_handle_clone).await {
                                            eprintln!("❌ Error sending busy message: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("❌ WebSocket handshake error from {}: {}", peer_str, e);
                            }
                        }
                    });
                }
                Err(e) => {
                    eprintln!("❌ Error accepting TCP connection: {}", e);
                    // continue accepting next connections
                }
            }
        }
    });
}


async fn reject_connection_busy(ws_stream: WebSocketStream<tokio::net::TcpStream>, app_handle: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    /// If the server is at capacity, we send a friendly JSON reply and close the socket.
    /// We accept the WebSocket handshake first (client expects it) then send this message.
    /// 
    /// 
    // split into writer/reader — we only need the writer to send the busy message
    let (mut write, _read) = ws_stream.split();

    let busy = json!({
        "status": "error",
        "message": "Server busy: too many connections"
    });

    if DEBUG_WS {println!("⛔ Rejecting connection: {}", busy);}
    emit_cep_status(&app_handle, "⛔ Connection Rejected: Server Busy.");


    // send busy message
    write.send(Message::Text(busy.to_string())).await?;

    // politely close the WebSocket (Close message)
    let _ = write.send(Message::Close(None)).await;

    Ok(())
}

/// Handles a single accepted & permitted WebSocket connection.
async fn handle_connection(
    ws_stream: WebSocketStream<tokio::net::TcpStream>,
    peer: String,
    app_handle: AppHandle,
    _permit: OwnedSemaphorePermit,
) -> Result<(), Box<dyn std::error::Error>> {

    /// We accept a concrete `WebSocketStream<tokio::net::TcpStream>` (the handshake has already been done).
    /// The argument `_permit: OwnedSemaphorePermit` is intentionally kept in the function signature:
    /// by holding it here (not dropping it), the permit remains active while the handler runs.
    /// When this function returns (or panics), `_permit` is dropped and the semaphore frees a slot.4
    /// 
    ///

    if DEBUG_WS {println!("✅ Client connected: {}", peer);}
    emit_cep_status(&app_handle, "✅ Connected.");



    // split into writer + reader halves (writer: Sink, reader: Stream)
    let (mut write, mut read) = ws_stream.split();

    // Send an initial "connected" handshake JSON
    let hello = json!({
        "status": "ok",
        "message": "Connected to Rust WS server"
    });
    write.send(Message::Text(hello.to_string())).await?;
    if DEBUG_WS {println!("Handshake to {}: {}", peer, hello);}
    

    // Loop reading messages from the client
    while let Some(msg_res) = read.next().await {
        let msg = msg_res?; // propagate tungstenite errors via ?
        match msg {
            Message::Text(text) => {
                // Received text frame — expected to be JSON containing { request_id?, command, payload }
                if DEBUG_WS {println!("Received from {}: {}", peer, text);}

                // Try to parse to our typed request. If parse fails, return an "Invalid JSON" reply.
                match serde_json::from_str::<WsRequest>(&text) {
                    Ok(req) => {
                        // Dispatch the command (async handler so we can await DB/cloud later)
                        let reply = handle_command(req, &app_handle).await;

                        // Serialize reply and send
                        let resp_text = serde_json::to_string(&reply)?;
                        if DEBUG_WS {println!("➡️ Sending to {}: {}", peer, resp_text);}
                        write.send(Message::Text(resp_text)).await?;

                    }
                    Err(_) => {
                        // Invalid JSON — reply with an error
                        let error = json!({
                            "status": "error",
                            "message": "Invalid JSON"
                        });
                        if DEBUG_WS {println!("Sending error to {}: {}", peer, error);}
                        write.send(Message::Text(error.to_string())).await?;
                    }
                }
            }
            Message::Close(_) => {
                println!("🔌 {} disconnected", peer);
                emit_cep_status(&app_handle, "🛑 Disconnected...");

                break;
            }
            Message::Ping(_) | Message::Pong(_) | Message::Binary(_) => {
                // ignore or handle if you expect binary frames or pings
            }
            _ => {}
        }
    }

    // When function ends, `_permit` gets dropped and the semaphore frees one slot.
    println!("🛑 Connection handler ended for {}", peer);
    
    Ok(())
}



//_______________PATHS________________________

/// Central async command dispatcher.
async fn handle_command(req: WsRequest, app_handle: &AppHandle) -> WsResponse {
    /// Add new commands here. Returns a typed WsResponse which will be serialized and sent back.
    ///
    /// Note: this function is `async` so you can `await` DB/HTTP/AI calls in handlers.
    /// 
    if DEBUG_WS {println!("Dispatching command: {} with payload: {}", req.command, req.payload);}

    match req.command.as_str() {
        "test_server_connection" => {
            emit_cep_status(app_handle, "✅ Connected (Server connection tested successfully).");
            WsResponse {
                request_id: req.request_id,
                status: "ok".into(),
                command: req.command,
                data: json!("Server is alive!"),
            }
        },

        "fetch_JSON" => WsResponse {
            request_id: req.request_id,
            status: "ok".into(),
            command: req.command,
            data: req.payload, // echo back the payload for this example
        },

        "fetch_deepFaceCameraEmotionList" => WsResponse {
            request_id: req.request_id,
            status: "ok".into(),
            command: req.command,
            data: json!(["happy", "sad", "angry"]),
        },

        // Unknown command
        other => WsResponse {
            request_id: req.request_id,
            status: "error".into(),
            command: other.to_string(),
            data: json!({ "message": "Unknown command" }),
        },
    }
}







//______________UI Events____________________
pub fn emit_status_event(app_handle: &AppHandle, event_name: &str, message: &str) {
    if let Err(e) = app_handle.emit(event_name, message) {
        eprintln!("Failed to emit {} event: {}", event_name, e);
    }
}

/// Predefined event emitter for CEP status updates
pub fn emit_cep_status(app_handle: &AppHandle, status: &str) {
    emit_status_event(app_handle, "cep-status", status);
}










