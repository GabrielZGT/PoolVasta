use crate::data_dragon::ChampionMeta;
use crate::lcu::LcuCredentials;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

const RECENT_GAMES_WINDOW: u32 = 20;
const RUSTY_AFTER_DAYS: i64 = 45;
const RUSTY_MIN_MASTERY_LEVEL: u32 = 5;
const POOL_MIN_MASTERY_LEVEL: u32 = 4;
const POOL_MIN_MASTERY_POINTS: u64 = 10_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MasteryEntry {
    champion_id: u32,
    champion_level: u32,
    champion_points: u64,
    highest_grade: Option<String>,
    last_play_time: i64, // epoch millis
}

#[derive(Debug, Deserialize)]
struct MatchHistoryResponse {
    games: GamesWrapper,
}

#[derive(Debug, Deserialize)]
struct GamesWrapper {
    games: Vec<GameEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameEntry {
    participant_identities: Vec<ParticipantIdentity>,
    participants: Vec<Participant>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParticipantIdentity {
    participant_id: u32,
    player: PlayerRef,
}

#[derive(Debug, Deserialize)]
struct PlayerRef {
    puuid: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Participant {
    participant_id: u32,
    champion_id: u32,
    stats: ParticipantStats,
}

#[derive(Debug, Deserialize)]
struct ParticipantStats {
    win: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolEntry {
    pub champion_id: u32,
    pub champion_name: String,
    pub mastery_level: u32,
    pub mastery_points: u64,
    /// Melhor nota alcançada nesta temporada com esse campeão. A LCU API não
    /// expõe a nota individual de partidas passadas nem uma "média" real —
    /// esse é o proxy de qualidade mais próximo disponível.
    pub highest_grade: Option<String>,
    pub games_recent: u32,
    pub wins_recent: u32,
    pub win_rate_recent: Option<f32>,
    pub days_since_last_played: i64,
    pub is_rusty: bool,
    pub score: f32,
}

async fn fetch_masteries(creds: &LcuCredentials) -> Result<Vec<MasteryEntry>, String> {
    let client = crate::lcu::build_client()?;
    let url = format!(
        "{}/lol-champion-mastery/v1/local-player/champion-mastery",
        creds.base_url()
    );

    let response = client
        .get(url)
        .basic_auth("riot", Some(&creds.token))
        .send()
        .await
        .map_err(|err| err.to_string())?;

    if !response.status().is_success() {
        return Err(format!("LCU respondeu com status {}", response.status()));
    }

    response
        .json::<Vec<MasteryEntry>>()
        .await
        .map_err(|err| err.to_string())
}

/// Conta jogos/vitórias recentes por campeão, olhando a janela das últimas
/// `RECENT_GAMES_WINDOW` partidas do jogador (não só as daquele campeão).
async fn fetch_recent_stats_by_champion(
    creds: &LcuCredentials,
    puuid: &str,
) -> Result<HashMap<u32, (u32, u32)>, String> {
    let client = crate::lcu::build_client()?;
    let url = format!(
        "{}/lol-match-history/v1/products/lol/current-summoner/matches?begIndex=0&endIndex={}",
        creds.base_url(),
        RECENT_GAMES_WINDOW.saturating_sub(1)
    );

    let response = client
        .get(url)
        .basic_auth("riot", Some(&creds.token))
        .send()
        .await
        .map_err(|err| err.to_string())?;

    if !response.status().is_success() {
        return Err(format!("LCU respondeu com status {}", response.status()));
    }

    let history = response
        .json::<MatchHistoryResponse>()
        .await
        .map_err(|err| err.to_string())?;

    let mut stats: HashMap<u32, (u32, u32)> = HashMap::new();

    for game in history.games.games {
        let my_participant_id = game
            .participant_identities
            .iter()
            .find(|identity| identity.player.puuid == puuid)
            .map(|identity| identity.participant_id);

        let Some(my_participant_id) = my_participant_id else {
            continue;
        };

        let Some(me) = game
            .participants
            .iter()
            .find(|participant| participant.participant_id == my_participant_id)
        else {
            continue;
        };

        let entry = stats.entry(me.champion_id).or_insert((0, 0));
        entry.0 += 1;
        if me.stats.win {
            entry.1 += 1;
        }
    }

    Ok(stats)
}

fn grade_score(grade: Option<&str>) -> f32 {
    match grade {
        Some("S+") => 1.0,
        Some("S") => 0.9,
        Some("S-") => 0.8,
        Some("A+") => 0.7,
        Some("A") => 0.6,
        Some("A-") => 0.5,
        Some("B+") => 0.35,
        Some("B") => 0.25,
        Some("B-") => 0.15,
        Some(_) => 0.05,
        None => 0.0,
    }
}

fn mastery_score(points: u64) -> f32 {
    (points as f32 / 200_000.0).min(1.0)
}

fn days_since(epoch_millis: i64) -> i64 {
    let now_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(epoch_millis);

    ((now_millis - epoch_millis).max(0)) / (1000 * 60 * 60 * 24)
}

pub async fn build_pool(
    creds: &LcuCredentials,
    puuid: &str,
    champion_names: &HashMap<u32, ChampionMeta>,
) -> Result<Vec<PoolEntry>, String> {
    let masteries = fetch_masteries(creds).await?;
    let recent_stats = fetch_recent_stats_by_champion(creds, puuid).await?;

    let mut entries: Vec<PoolEntry> = masteries
        .into_iter()
        .filter(|m| {
            m.champion_level >= POOL_MIN_MASTERY_LEVEL || m.champion_points >= POOL_MIN_MASTERY_POINTS
        })
        .map(|m| {
            let (games_recent, wins_recent) = recent_stats.get(&m.champion_id).copied().unwrap_or((0, 0));
            let win_rate_recent = if games_recent > 0 {
                Some(wins_recent as f32 / games_recent as f32)
            } else {
                None
            };
            let days = days_since(m.last_play_time);
            let is_rusty = days >= RUSTY_AFTER_DAYS && m.champion_level >= RUSTY_MIN_MASTERY_LEVEL;

            let win_rate_component = win_rate_recent.unwrap_or(0.5);
            let score = win_rate_component * 0.4
                + mastery_score(m.champion_points) * 0.35
                + grade_score(m.highest_grade.as_deref()) * 0.25;

            let champion_name = champion_names
                .get(&m.champion_id)
                .map(|meta| meta.name.clone())
                .unwrap_or_else(|| format!("Campeão #{}", m.champion_id));

            PoolEntry {
                champion_id: m.champion_id,
                champion_name,
                mastery_level: m.champion_level,
                mastery_points: m.champion_points,
                highest_grade: m.highest_grade,
                games_recent,
                wins_recent,
                win_rate_recent,
                days_since_last_played: days,
                is_rusty,
                score,
            }
        })
        .collect();

    entries.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    Ok(entries)
}
