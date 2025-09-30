//deepFaceProcess.rs

use once_cell::sync::OnceCell;
use serde_json::{json, Value};


use std::sync::Mutex;
use std::path::PathBuf;
use std::time::Duration;
use std::process::{Child, Stdio}; // std::process Command direct conflict with tokio::processCommand
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::oneshot;

use tokio_tungstenite::{
    connect_async, 
    tungstenite::protocol::Message, 
    MaybeTlsStream, 
    WebSocketStream
    };

use futures_util::{SinkExt, StreamExt};


// ---------------------------------------
// Globals
static DEEPFACE_PROCESS: OnceCell<Mutex<Option<tokio::process::Child>>> = OnceCell::new();
static WS_CLIENT: OnceCell<AsyncMutex<WebSocketStream<MaybeTlsStream<TcpStream>>>> = OnceCell::new();

pub const DEBUG_DEEPFACE: bool = true;
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

//------------------
//    Functions
// -----------------

#[tauri::command]
pub async fn start_deepface_server(port: u16) -> Result<(), String> {

    // Check if deepface instance already running
    if DEEPFACE_PROCESS.get().is_some() {return Err("DeepFace server already started".into());}

    if DEBUG_DEEPFACE {println!("[Rust] Starting DeepFace server...");}

    // Resolve exe path & Include "_internal" dependencies floder.
    let mut exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get current exe path: {}", e))?;
    exe_path.pop(); // remove app exe name
    exe_path.push("binaries");
    exe_path.push("deepface_cli");
    exe_path.push("deepface_cli.exe");

    let exe_dir: PathBuf = exe_path.parent().unwrap().to_path_buf();

    // Build args
    let args = vec![
        "serve".to_string(),
        "--host".to_string(),
        "127.0.0.1".to_string(),
        "--port".to_string(),
        port.to_string(),
    ];

    if DEBUG_DEEPFACE {
        println!("Running DeepFace exe at: {:?}", exe_path);
        println!("With args: {:?}", args);
    }

    // Spawn process (tokio::process)
    let mut child = Command::new(&exe_path)
        .args(&args)
        .current_dir(&exe_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start deepface_cli: {}", e))?;

    // Read stdIO
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // oneshot channel to signal readiness
    let (ready_tx, ready_rx) = oneshot::channel();

    // Spawn stdout reader
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            println!("[deepface_cli stdout] {}", line);
        }
    });

    // ---------- stderr reader ----------
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            eprintln!("[deepface_cli stderr] {}", line);
            // LOOK FOR THE SUCCESS STRING HERE
            if line.contains("WebSocket server started successfully") {
                let _ = ready_tx.send(());   // <- signal parent
                break;                       // optional: stop scanning once signaled
            }
        }
    });

    // Store process handle
    DEEPFACE_PROCESS.set(Mutex::new(Some(child))).ok();

    // Wait for "DeepFace serve mode started"
    tokio::time::timeout(Duration::from_secs(60), ready_rx)
        .await
        .map_err(|_| "Timeout waiting for DeepFace to start".to_string())?
        .map_err(|_| "DeepFace startup signal failed".to_string())?;

    // Now connect WS
    let url = format!("ws://127.0.0.1:{}", port);
    let (ws_stream, _) = connect_async(&url)
        .await
        .map_err(|e| format!("Failed to connect WS: {}", e))?;

    WS_CLIENT.set(AsyncMutex::new(ws_stream)).ok();

    if DEBUG_DEEPFACE {println!("[Rust] deepface_cli.exe started and WS connected on port {}", port);}

    Ok(())
}




#[tauri::command]
pub async fn stop_deepface_server() -> Result<(), String> {
    if let Some(proc_mutex) = DEEPFACE_PROCESS.get() {
        let mut lock = proc_mutex.lock().unwrap();
        if let Some(child) = lock.as_mut() {
            child.kill().await.map_err(|e| format!("Failed to kill deepface_cli: {}", e))?;
            if DEBUG_DEEPFACE {
                println!("[Rust] deepface_cli.exe stopped.");
            }
            *lock = None;
        }
    }
    Ok(())
}


// Helpers
fn next_request_id() -> u64 {
    REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst)
}

async fn send_request(req: Value) -> Result<Value, String> {
    let client_mutex = WS_CLIENT.get().ok_or("DeepFace WS not started")?;
    let mut client = client_mutex.lock().await;

    let text = req.to_string();
    if DEBUG_DEEPFACE {
        println!("[Rust → WS] {}", text);
    }

    client
        .send(Message::Text(text))
        .await
        .map_err(|e| e.to_string())?;

    if let Some(msg) = client.next().await {
        match msg {
            Ok(Message::Text(resp)) => {
                if DEBUG_DEEPFACE {
                    println!("[WS → Rust] {}", resp);
                }
                let val: Value = serde_json::from_str(&resp).map_err(|e| e.to_string())?;
                Ok(val)
            }
            Ok(other) => Err(format!("Unexpected WS message: {:?}", other)),
            Err(e) => Err(format!("WS error: {}", e)),
        }
    } else {
        Err("No response from DeepFace".into())
    }
}

//------------------
//    Commands
// -----------------

#[tauri::command]
pub async fn analyze_deepface(
    frame: String,
    actions: String,
    detector: Option<String>,
    model: Option<String>,
) -> Result<Value, String> {
    let req = json!({
        "requestId": next_request_id(),
        "cmd": "analyze",
        "frame": frame,
        "actions": actions,
        "detector": detector,
        "model": model
    });

    // if DEBUG_DEEPFACE {println("")}
    send_request(req).await
}

#[tauri::command]
pub async fn verify_deepface(
    img1: String,
    img2: String,
    detector: Option<String>,
    model: Option<String>,
) -> Result<Value, String> {
    let req = json!({
        "requestId": next_request_id(),
        "cmd": "verify",
        "img1": img1,
        "img2": img2,
        "detector": detector,
        "model": model
    });
    send_request(req).await
}

#[tauri::command]
pub async fn detect_deepface(frame: String, detector: Option<String>) -> Result<Value, String> {
    let req = json!({
        "requestId": next_request_id(),
        "cmd": "detect",
        "frame": frame,
        "detector": detector
    });
    send_request(req).await
}






// ----------------------------------------------------------------

// Run deepface_cli.exe with arguments and capture JSON output.
// pub fn OLD_run_deepface_command(args: Vec<String>) -> Result<Value, String> {
//     if DEBUG_DEEPFACE {
//         println!("Sending Analysis to Deepface...");
//     }

//     // Resolve exe path (inside binaries folder next to app exe)
//     let mut exe_path = std::env::current_exe()
//         .map_err(|e| format!("Failed to get current exe path: {}", e))?;
//         exe_path.pop(); // remove app exe name
//         exe_path.push("binaries");
//         exe_path.push("deepface_cli");
//         exe_path.push("deepface_cli.exe");

//     let exe_dir: PathBuf = exe_path.parent().unwrap().to_path_buf();

//     if DEBUG_DEEPFACE {
//         println!("Running DeepFace exe at: {:?}", exe_path);
//         println!("With args: {:?}", args);
//         // println!("Working dir: {:?}", exe_dir);
//         // println!("Exists exe? {}", exe_path.exists());
//         // println!("Exists _internal ? {}", exe_dir.join("_internal").exists());
//         // println!("Exists internal dll? {}", exe_dir.join("_internal/python312.dll").exists());
//         // println!("Debug PATH: {}", format!(
//         // "{};{}",
//         // exe_dir.join("_internal").display(),
//         // std::env::var("PATH").unwrap_or_default()
//         // ));
//     }

//     let mut child = Command::new(&exe_path)
//         .args(&args)
//         .stdout(Stdio::piped())
//         .stderr(Stdio::piped())
//         // .current_dir(&exe_dir)
//         // .env("PYTHONHOME", &exe_dir)
//         // .env("PYTHON_DLL_PATH", exe_dir.join("_internal"))

//         // .env("PYTHONHOME", exe_dir.join("_internal")) // ensure Python DLLs are found
//         // .env("PYTHONPATH", exe_dir.join("_internal"))
//         // .env("PATH", format!(
//         //     "{};{}", 
//         //     exe_dir.join("_internal").display(), 
//         //     std::env::var("PATH").unwrap()
//         // ))
//         .spawn()
//         .map_err(|e| format!("Failed to spawn deepface_cli: {}", e))?;

//     // Capture stdout
//     let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
//     let reader = BufReader::new(stdout);
//     let mut output_str = String::new();
//     for line in reader.lines() {
//         let l = line.unwrap_or_default();
//         if DEBUG_DEEPFACE {println!("DeepFace stdout: {}", l);}
//         output_str.push_str(&l);
//     }

//     // Capture stderr
//     let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;
//     let stderr_reader = BufReader::new(stderr);
//     let mut err_output = String::new();
//     for line in stderr_reader.lines() {
//         let l = line.unwrap_or_default();
//         if DEBUG_DEEPFACE {
//             println!("DeepFace stderr: {}", l);
//         }
//         err_output.push_str(&l);
//         err_output.push('\n');
//     }

//     // Wait for process to finish
//     let status = child.wait()
//         .map_err(|e| format!("Failed to wait for deepface_cli: {}", e))?;

//     if !status.success() {
//         return Err(format!(
//             "deepface_cli failed with exit code: {:?}\nStderr: {}",
//             status.code(),
//             err_output
//         ));
//     }

//     // Parse JSON output
//     let parsed: Value = serde_json::from_str(&output_str)
//         .map_err(|e| format!(
//             "Failed to parse deepface_cli JSON: {}\nOutput: {}\nStderr: {}",
//             e, output_str, err_output
//         ))?;

//     if DEBUG_DEEPFACE {
//         println!("Deepface response: {:?}", parsed);
//     }

//     Ok(parsed)
// }

// #[tauri::command]
// pub async fn OLD_analyze_deepface(
//     frames: Vec<String>,
//     actions: String,
//     model: Option<String>,
//     detector: Option<String>,
// ) -> Result<Value, String> {
//     let mut args = vec![
//         "analyze".to_string(),
//         "--frames".to_string(),
//     ];

//     // Add frames
//     args.extend(frames);

//     // Actions (mandatory)
//     args.push("--actions".to_string());
//     args.push(actions);

//     // Model (optional)
//     if let Some(m) = model {
//         args.push("--model".to_string());
//         args.push(m);
//     }

//     // Detector (optional)
//     if let Some(d) = detector {
//         args.push("--detector".to_string());
//         args.push(d);
//     }

//     if DEBUG_DEEPFACE {
//         println!("Command: {:?}", args);
//     }

//     run_deepface_command(args)
// }
