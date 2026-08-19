mod champ_select;
mod data_dragon;
mod lcu;
mod pool;

use champ_select::ChampSelectStatus;
use data_dragon::ChampionMeta;
use lcu::LcuStatus;
use pool::PoolEntry;
use std::collections::HashMap;
use std::time::Duration;
use sysinfo::System;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

#[derive(Default)]
struct AppState {
    champion_names: Mutex<Option<HashMap<u32, ChampionMeta>>>,
}

async fn champion_names(state: &AppState) -> Result<HashMap<u32, ChampionMeta>, String> {
    let mut guard = state.champion_names.lock().await;
    if guard.is_none() {
        *guard = Some(data_dragon::fetch_champion_map().await?);
    }
    Ok(guard.clone().unwrap())
}

#[tauri::command]
async fn get_lcu_status() -> LcuStatus {
    let mut system = System::new();
    lcu::current_status(&mut system).await
}

#[tauri::command]
async fn get_champion_pool(state: tauri::State<'_, AppState>) -> Result<Vec<PoolEntry>, String> {
    let mut system = System::new();
    let creds = lcu::find_credentials(&mut system).ok_or("Cliente do League não encontrado")?;
    let summoner = lcu::fetch_current_summoner(&creds).await?;
    let names = champion_names(&state).await?;
    pool::build_pool(&creds, &summoner.puuid, &names).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![get_lcu_status, get_champion_pool])
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

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut system = System::new();
                loop {
                    let mut status = match lcu::find_credentials(&mut system) {
                        Some(creds) => champ_select::fetch_status(&creds).await,
                        None => ChampSelectStatus::NotInChampSelect,
                    };

                    if let ChampSelectStatus::InChampSelect { session } = &mut status {
                        let state = handle.state::<AppState>();
                        if let Ok(names) = champion_names(state.inner()).await {
                            for player in session.my_team.iter_mut().chain(session.their_team.iter_mut()) {
                                if player.champion_id != 0 {
                                    player.champion_name =
                                        names.get(&player.champion_id).map(|meta| meta.name.clone());
                                }
                            }
                        }
                    }

                    let _ = handle.emit("champ-select-status", &status);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
