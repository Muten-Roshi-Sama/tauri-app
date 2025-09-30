// src/database.rs
pub fn init_db() {
    println!("🟢 init_db called");
}

pub fn add_clip(path: &str) {
    println!("🟢 add_clip called with path: {}", path);
}

pub fn add_marker(clip_id: i32, timestamp: f64) {
    println!("🟢 add_marker to clip {} at {}", clip_id, timestamp);
}

pub fn list_markers(clip_id: i32) {
    println!("🟢 list_markers called for clip {}", clip_id);
}

pub fn delete_marker(marker_id: i32) {
    println!("🟢 delete_marker called for marker {}", marker_id);
}
