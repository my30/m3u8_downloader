// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod download;

use std::sync::Arc;
use download::DownloadManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(DownloadManager::default()))
        .invoke_handler(tauri::generate_handler![
            download::start_download,
            download::cancel_download,
            download::get_queue,
            download::clear_completed,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
