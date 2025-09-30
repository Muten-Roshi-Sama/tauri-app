// Tauri and plugin APIs
use tauri::{App, Manager};

// Import our own modules
mod commands;
mod license;
mod database;
mod websocket;
mod deepFaceProcess;

use crate::license::start_license_checker;
use crate::deepFaceProcess::start_deepface_server;
use crate::deepFaceProcess::analyze_deepface;
use crate::deepFaceProcess::verify_deepface;
use crate::deepFaceProcess::detect_deepface;

// ----------------- App Entry -----------------

// This is the entry point of the Tauri app
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()

        // PLUGINS
        .plugin(tauri_plugin_opener::init())
        

        // FRONTEND Commands
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::add_marker,
            start_deepface_server,        //? NOT a command, no prefix
            analyze_deepface,
            verify_deepface,
            detect_deepface
        ])

        // Code Running at startup
        .setup(|app| {
            
            // WEBSOCKET
            websocket::start_websocket_server(app.handle().clone());

            // Start background license checker when app launches
            start_license_checker(app.handle().clone()); 

            Ok(())
        })

        // Build & run app
        .run(tauri::generate_context!()) 
        .expect("error while running tauri application");
}
