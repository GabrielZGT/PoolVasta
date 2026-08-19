use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChampionMeta {
    pub id: String, // ex: "Ashe" — usado nas URLs de ícone da CDN
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct ChampionJsonEntry {
    key: String, // championId numérico, como string
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct ChampionJsonResponse {
    data: HashMap<String, ChampionJsonEntry>,
}

pub async fn fetch_champion_map() -> Result<HashMap<u32, ChampionMeta>, String> {
    let client = reqwest::Client::new();

    let versions: Vec<String> = client
        .get("https://ddragon.leagueoflegends.com/api/versions.json")
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json()
        .await
        .map_err(|err| err.to_string())?;

    let latest = versions
        .first()
        .ok_or_else(|| "nenhuma versão do Data Dragon encontrada".to_string())?;

    let url = format!("https://ddragon.leagueoflegends.com/cdn/{latest}/data/en_US/champion.json");

    let parsed: ChampionJsonResponse = client
        .get(url)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json()
        .await
        .map_err(|err| err.to_string())?;

    let mut map = HashMap::with_capacity(parsed.data.len());
    for entry in parsed.data.into_values() {
        if let Ok(numeric_id) = entry.key.parse::<u32>() {
            map.insert(
                numeric_id,
                ChampionMeta {
                    id: entry.id,
                    name: entry.name,
                },
            );
        }
    }

    Ok(map)
}
