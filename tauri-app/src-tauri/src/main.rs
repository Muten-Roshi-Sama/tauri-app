// CODE Architecture : 
// src-tauri/
//  ├── src/
//  │   ├── main.rs       # <- entrypoint
//  │   ├── lib.rs        # <- app wiring (setup, plugins, invoke_handler)
//  │   ├── commands.rs   # <- frontend-callable functions
//  │   └── license.rs    # <- license validation logic + background thread
//  │   └── websocket.rs   # <- manage communication with CEP
//  │   └── database.rs 

//  │   └── deepFaceProcess.rs    # <- spawn deepface AI, send commands to it




// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    _tauri_local_lib::run()
}
