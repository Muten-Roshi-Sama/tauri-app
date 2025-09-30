//! Make sure commands are public
//TODO: pub might be too exposed, keep frontend commands here only

// ----------------- Commands -----------------

// This is a Tauri command callable from JS (frontend).
// Example: `invoke("greet", { name: "Alice" })`
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}


//_________CEP____________

#[tauri::command]
pub fn add_marker(timestamp: f64) {
    println!("🟢 add_marker called at timestamp: {}", timestamp);
}
