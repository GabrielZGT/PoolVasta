mod champ_select;
mod data_dragon;
mod lcu;
mod pool;
mod riot_api;
mod suggest;

use champ_select::{ChampSelectPlayer, ChampSelectStatus};
use data_dragon::ChampionMeta;
use lcu::LcuStatus;
use pool::PoolEntry;
use riot_api::PlayerIntel;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use suggest::SuggestedPick;
use sysinfo::System;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

#[derive(Default)]
struct AppState {
    champion_names: Mutex<Option<HashMap<u32, ChampionMeta>>>,
    /// Pool calculada uma vez por sessão de champion select (evita recomputar a
    /// cada tick do polling — pool não muda no meio da mesma partida).
    cached_pool: Mutex<Option<Vec<PoolEntry>>>,
}

async fn ensure_pool(
    state: &AppState,
    creds: &lcu::LcuCredentials,
    names: &HashMap<u32, ChampionMeta>,
) -> Result<Vec<PoolEntry>, String> {
    let mut guard = state.cached_pool.lock().await;
    if guard.is_none() {
        let summoner = lcu::fetch_current_summoner(creds).await?;
        *guard = Some(pool::build_pool(creds, &summoner.puuid, names).await?);
    }
    Ok(guard.clone().unwrap())
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

#[tauri::command]
fn has_riot_api_key(app: AppHandle) -> bool {
    riot_api::get_api_key(&app).is_some()
}

#[tauri::command]
fn set_riot_api_key(app: AppHandle, key: String) -> Result<(), String> {
    riot_api::save_api_key(&app, &key)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TeamIntel {
    ally: Vec<PlayerIntel>,
    enemy: Vec<PlayerIntel>,
}

fn champion_name_or_id(names: &HashMap<u32, ChampionMeta>, champion_id: u32) -> String {
    names
        .get(&champion_id)
        .map(|meta| meta.name.clone())
        .unwrap_or_else(|| format!("Campeão #{champion_id}"))
}

async fn build_player_intel(
    player: &ChampSelectPlayer,
    api_key: &str,
    platform: &str,
    regional: &str,
    names: &HashMap<u32, ChampionMeta>,
) -> Option<PlayerIntel> {
    let masteries = riot_api::fetch_masteries_by_puuid(api_key, platform, &player.puuid)
        .await
        .ok()?;
    let mastery = riot_api::analyze_masteries(&masteries)?;

    let recent_champions = riot_api::fetch_recent_champions(api_key, regional, &player.puuid)
        .await
        .unwrap_or_default();
    let recent = riot_api::analyze_recent(&recent_champions);

    let is_specialist = mastery.is_lifetime_one_trick || recent.map(|(_, _, _, flag)| flag).unwrap_or(false);

    // Prioriza o campeão do sinal recente (mais acionável pra "banir agora") quando ele
    // também bateu o critério; senão cai pro campeão de maior maestria vitalícia.
    let (suggested_id, suggested_name) = match recent {
        Some((champion_id, _, _, true)) => (champion_id, champion_name_or_id(names, champion_id)),
        _ => (
            mastery.champion_id,
            champion_name_or_id(names, mastery.champion_id),
        ),
    };

    Some(PlayerIntel {
        cell_id: player.cell_id,
        lifetime_top_champion_id: mastery.champion_id,
        lifetime_top_champion_name: champion_name_or_id(names, mastery.champion_id),
        lifetime_top_champion_points: mastery.points,
        lifetime_top_champion_level: mastery.level,
        lifetime_share: mastery.share,
        recent_champion_id: recent.map(|(id, ..)| id),
        recent_champion_name: recent.map(|(id, ..)| champion_name_or_id(names, id)),
        recent_games_analyzed: recent.map(|(_, _, total, _)| total).unwrap_or(0),
        recent_games_on_top: recent.map(|(_, on_top, _, _)| on_top).unwrap_or(0),
        is_specialist,
        suggested_ban_champion_id: suggested_id,
        suggested_ban_champion_name: suggested_name,
    })
}

#[tauri::command]
async fn get_champ_select_intel(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<TeamIntel, String> {
    let api_key = riot_api::get_api_key(&app)
        .ok_or("Configure sua Riot API key nas configurações antes de analisar o time")?;

    let mut system = System::new();
    let creds = lcu::find_credentials(&mut system).ok_or("Cliente do League não encontrado")?;
    let platform = riot_api::platform_host(&creds.region)
        .ok_or_else(|| format!("Região '{}' não reconhecida", creds.region))?;
    let regional = riot_api::regional_host(&creds.region)
        .ok_or_else(|| format!("Região '{}' não reconhecida", creds.region))?;

    let status = champ_select::fetch_status(&creds).await;
    let ChampSelectStatus::InChampSelect { session } = status else {
        return Err("Você não está em champion select agora".to_string());
    };

    let my_summoner = lcu::fetch_current_summoner(&creds).await?;
    let names = champion_names(&state).await?;

    let mut ally = Vec::new();
    for player in session
        .my_team
        .iter()
        .filter(|p| !p.puuid.is_empty() && p.puuid != my_summoner.puuid)
    {
        if let Some(intel) = build_player_intel(player, &api_key, platform, regional, &names).await {
            ally.push(intel);
        }
    }

    let mut enemy = Vec::new();
    for player in session.their_team.iter().filter(|p| !p.puuid.is_empty()) {
        if let Some(intel) = build_player_intel(player, &api_key, platform, regional, &names).await {
            enemy.push(intel);
        }
    }

    Ok(TeamIntel { ally, enemy })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_lcu_status,
            get_champion_pool,
            has_riot_api_key,
            set_riot_api_key,
            get_champ_select_intel
        ])
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
                    let creds = lcu::find_credentials(&mut system);
                    let mut status = match &creds {
                        Some(creds) => champ_select::fetch_status(creds).await,
                        None => ChampSelectStatus::NotInChampSelect,
                    };

                    let state = handle.state::<AppState>();
                    let mut suggestions: Vec<SuggestedPick> = Vec::new();

                    if let ChampSelectStatus::InChampSelect { session } = &mut status {
                        if let Ok(names) = champion_names(state.inner()).await {
                            for player in session.my_team.iter_mut().chain(session.their_team.iter_mut()) {
                                if player.champion_id != 0 {
                                    player.champion_name =
                                        names.get(&player.champion_id).map(|meta| meta.name.clone());
                                }
                            }

                            if let Some(creds) = &creds {
                                let ally_ids: Vec<u32> =
                                    session.my_team.iter().map(|p| p.champion_id).collect();
                                let mut unavailable: HashSet<u32> =
                                    session.banned_champion_ids.iter().copied().collect();
                                for player in session.my_team.iter().chain(session.their_team.iter()) {
                                    if player.champion_id != 0 {
                                        unavailable.insert(player.champion_id);
                                    }
                                }

                                if let Ok(pool_entries) = ensure_pool(state.inner(), creds, &names).await {
                                    suggestions =
                                        suggest::suggest(&pool_entries, &names, &unavailable, &ally_ids);
                                }
                            }
                        }
                    } else {
                        state.cached_pool.lock().await.take();
                    }

                    let _ = handle.emit("champ-select-status", &status);
                    let _ = handle.emit("champ-select-suggestions", &suggestions);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
