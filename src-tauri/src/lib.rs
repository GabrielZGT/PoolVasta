mod lcu;

use lcu::LcuStatus;
use std::time::Duration;
use sysinfo::System;
use tauri::Emitter;

#[tauri::command]
async fn get_lcu_status() -> LcuStatus {
    let mut system = System::new();
    lcu::current_status(&mut system).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_lcu_status])
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut system = System::new();
                loop {
                    let status = lcu::current_status(&mut system).await;
                    let _ = handle.emit("lcu-status", &status);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
