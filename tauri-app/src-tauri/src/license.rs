// Import traits and libraries
use tauri::{Emitter, Manager}; // Tauri tools: Manager lets us access app state, Emitter lets us send events to frontend
use reqwest::blocking::Client; // Reqwest = HTTP client (blocking means synchronous calls)
use serde::Deserialize;      // parse JSON responses into Rust structs
use std::time::Duration;       // For sleep


//____________Const___________
pub const CLOUD_ADDRESS: &str = "http://localhost:3000";
pub const DEBUG_LICENSE: bool = false;
pub const SLEEP_INTERVAL: u64 = 20; /// Sleep interval between license checks (seconds)


//_____________Struct _________________________
// Example server response: { "success": true, "message": "✅ License valid" }
#[derive(Deserialize, Debug)]
struct ValidateResponse {
    success: bool,
    message: String,
}


//_____________fn ____________________________

// Function to send license key to the server and get result
fn validate_license(key: &str, app_handle: &tauri::AppHandle) -> Result<String, String> {
    // Create an HTTP client
    let client = Client::new();

    if DEBUG_LICENSE {println!("Sending license key to the cloud server...");}

    // Send POST request to cloud server with { "key": key }
    let res = client
        .post(&format!("{}/validate", CLOUD_ADDRESS))
        .json(&serde_json::json!({ "key": key }))
        .send();

    // Handle server response
    let result = match res {
        Ok(resp) => {
            // If HTTP status is success (200 OK)
            if resp.status().is_success() {
                // Try parsing JSON into our ValidateResponse struct
                let parsed: ValidateResponse = resp.json().unwrap_or(ValidateResponse {
                    success: false,
                    message: "Parse error".to_string(),
                });

                // debug: show parsed response
                if DEBUG_LICENSE {println!("Server response: {:?}", parsed);}

                // Return Ok if license is valid, else Err
                if parsed.success {Ok(parsed.message)} 
                else {Err(parsed.message)}

            } else {
                // Non-200 response (like 403, 500…)
                Err(format!("HTTP error: {}", resp.status()))
            }
        }
        // Network failure (server down, no internet…)
        Err(err) => {
            eprintln!("❌ Network error while validating license: {}", err);
            Err(format!("Network error: {}", err))
        }
    };

    // Emit the result regardless of success/failure
    match &result {
        Ok(msg) => {
            let _ = app_handle.emit("status-tauri-cloud", msg);
        }
        Err(err) => {
            let _ = app_handle.emit("status-tauri-cloud", err);
        }
    }
    result
}



// This function runs in a separate thread and checks license every 5s
pub fn start_license_checker(app_handle: tauri::AppHandle) {
    let key = "TEST-123"; // ⚠️ TODO: replace later with config or user input

    

    // Spawn a background thread so it doesn’t block the main app
    std::thread::spawn(move || {


        std::thread::sleep(Duration::from_secs(2)); // let UI time to register
        let _ = validate_license(key, &app_handle); // Initial Check (startup)



        loop {
            
            std::thread::sleep(Duration::from_secs(SLEEP_INTERVAL)); // Sleep 5 seconds before checking again
            let _ = validate_license(key, &app_handle); // Call license validator
        }
    });
}











