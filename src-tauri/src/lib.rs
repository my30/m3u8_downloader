mod download;

use std::sync::Arc;

use download::DownloadManager;
use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(DownloadManager::default()))
        .invoke_handler(tauri::generate_handler![
            download::start_download,
            download::cancel_download,
            download::get_queue,
            download::clear_completed,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|handle, event| {
        // Don't leave orphaned ffmpeg processes downloading after the window closes.
        if let RunEvent::Exit = event {
            handle.state::<Arc<DownloadManager>>().kill_all();
        }
    });
}
