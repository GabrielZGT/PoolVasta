use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredConfig {
    riot_api_key: Option<String>,
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir.join("config.json"))
}

fn load_config(app: &AppHandle) -> StoredConfig {
    let Ok(path) = config_path(app) else {
        return StoredConfig::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

pub fn get_api_key(app: &AppHandle) -> Option<String> {
    load_config(app).riot_api_key
}

pub fn save_api_key(app: &AppHandle, key: &str) -> Result<(), String> {
    let path = config_path(app)?;
    let config = StoredConfig {
        riot_api_key: Some(key.trim().to_string()),
    };
    let json = serde_json::to_string_pretty(&config).map_err(|err| err.to_string())?;
    std::fs::write(path, json).map_err(|err| err.to_string())
}

/// Mapa de região do cliente (como aparece em `--region=` na linha de comando)
/// pra host de platform routing da API oficial da Riot (mastery-v4).
pub fn platform_host(region: &str) -> Option<&'static str> {
    match region.to_uppercase().as_str() {
        "BR" => Some("br1"),
        "NA" => Some("na1"),
        "EUW" => Some("euw1"),
        "EUNE" => Some("eun1"),
        "KR" => Some("kr"),
        "JP" => Some("jp1"),
        "LAN" => Some("la1"),
        "LAS" => Some("la2"),
        "OCE" => Some("oc1"),
        "TR" => Some("tr1"),
        "RU" => Some("ru"),
        "PH" => Some("ph2"),
        "SG" => Some("sg2"),
        "TH" => Some("th2"),
        "TW" => Some("tw2"),
        "VN" => Some("vn2"),
        _ => None,
    }
}

/// Mapa de região pra host de regional routing (match-v5, account-v1).
pub fn regional_host(region: &str) -> Option<&'static str> {
    match region.to_uppercase().as_str() {
        "BR" | "LAN" | "LAS" | "NA" | "OCE" => Some("americas"),
        "EUW" | "EUNE" | "TR" | "RU" => Some("europe"),
        "KR" | "JP" => Some("asia"),
        "PH" | "SG" | "TH" | "TW" | "VN" => Some("sea"),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiotMasteryEntry {
    pub champion_id: u32,
    pub champion_level: u32,
    pub champion_points: u64,
}

pub async fn fetch_masteries_by_puuid(
    api_key: &str,
    platform: &str,
    puuid: &str,
) -> Result<Vec<RiotMasteryEntry>, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://{platform}.api.riotgames.com/lol/champion-mastery/v4/champion-masteries/by-puuid/{puuid}"
    );

    let response = client
        .get(url)
        .header("X-Riot-Token", api_key)
        .send()
        .await
        .map_err(|err| err.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Riot API respondeu com status {}", response.status()));
    }

    response
        .json::<Vec<RiotMasteryEntry>>()
        .await
        .map_err(|err| err.to_string())
}

const RECENT_MATCHES_TO_CHECK: u32 = 8;

#[derive(Debug, Deserialize)]
struct MatchInfo {
    participants: Vec<MatchParticipant>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchParticipant {
    puuid: String,
    champion_id: u32,
}

#[derive(Debug, Deserialize)]
struct MatchDetailResponse {
    info: MatchInfo,
}

/// Busca o campeão jogado por `puuid` em cada uma das últimas `RECENT_MATCHES_TO_CHECK`
/// partidas (independente da fila). Cada partida exige uma chamada extra pra API — por
/// isso o número de jogos é pequeno, pra caber no rate limit de uma chave pessoal mesmo
/// analisando vários jogadores numa única champion select.
pub async fn fetch_recent_champions(
    api_key: &str,
    regional: &str,
    puuid: &str,
) -> Result<Vec<u32>, String> {
    let client = reqwest::Client::new();

    let ids_url = format!(
        "https://{regional}.api.riotgames.com/lol/match/v5/matches/by-puuid/{puuid}/ids?start=0&count={RECENT_MATCHES_TO_CHECK}"
    );
    let ids_response = client
        .get(ids_url)
        .header("X-Riot-Token", api_key)
        .send()
        .await
        .map_err(|err| err.to_string())?;

    if !ids_response.status().is_success() {
        return Err(format!("Riot API respondeu com status {}", ids_response.status()));
    }

    let match_ids: Vec<String> = ids_response.json().await.map_err(|err| err.to_string())?;

    let mut champions = Vec::with_capacity(match_ids.len());
    for match_id in match_ids {
        let match_url = format!("https://{regional}.api.riotgames.com/lol/match/v5/matches/{match_id}");
        let Ok(response) = client.get(match_url).header("X-Riot-Token", api_key).send().await else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let Ok(detail) = response.json::<MatchDetailResponse>().await else {
            continue;
        };
        if let Some(me) = detail.info.participants.iter().find(|p| p.puuid == puuid) {
            champions.push(me.champion_id);
        }
    }

    Ok(champions)
}

const LIFETIME_SHARE_THRESHOLD: f32 = 0.5;
const LIFETIME_MIN_LEVEL: u32 = 6;
const RECENT_MIN_GAMES: usize = 4;
const RECENT_SHARE_THRESHOLD: f32 = 0.6;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerIntel {
    pub cell_id: i64,
    pub lifetime_top_champion_id: u32,
    pub lifetime_top_champion_name: String,
    pub lifetime_top_champion_points: u64,
    pub lifetime_top_champion_level: u32,
    pub lifetime_share: f32,
    pub recent_champion_id: Option<u32>,
    pub recent_champion_name: Option<String>,
    pub recent_games_analyzed: u32,
    pub recent_games_on_top: u32,
    pub is_specialist: bool,
    pub suggested_ban_champion_id: u32,
    pub suggested_ban_champion_name: String,
}

pub struct MasteryAnalysis {
    pub champion_id: u32,
    pub points: u64,
    pub level: u32,
    pub share: f32,
    pub is_lifetime_one_trick: bool,
}

pub fn analyze_masteries(masteries: &[RiotMasteryEntry]) -> Option<MasteryAnalysis> {
    let top = masteries.iter().max_by_key(|m| m.champion_points)?;
    let total: u64 = masteries.iter().map(|m| m.champion_points).sum();
    if total == 0 {
        return None;
    }
    let share = top.champion_points as f32 / total as f32;
    let is_lifetime_one_trick = share >= LIFETIME_SHARE_THRESHOLD && top.champion_level >= LIFETIME_MIN_LEVEL;

    Some(MasteryAnalysis {
        champion_id: top.champion_id,
        points: top.champion_points,
        level: top.champion_level,
        share,
        is_lifetime_one_trick,
    })
}

/// Acha o campeão mais jogado numa lista de partidas recentes e se ele domina o suficiente
/// pra ser considerado "especialista atual" (não precisa ser one-trick a vida toda).
pub fn analyze_recent(champion_ids: &[u32]) -> Option<(u32, u32, u32, bool)> {
    if champion_ids.is_empty() {
        return None;
    }

    let mut counts: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    for &id in champion_ids {
        *counts.entry(id).or_insert(0) += 1;
    }

    let (&champion_id, &games_on_top) = counts.iter().max_by_key(|(_, count)| **count)?;
    let total = champion_ids.len();
    let share = games_on_top as f32 / total as f32;
    let is_specialist = total >= RECENT_MIN_GAMES && share >= RECENT_SHARE_THRESHOLD;

    Some((champion_id, games_on_top, total as u32, is_specialist))
}
