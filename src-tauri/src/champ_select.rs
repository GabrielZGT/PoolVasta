use crate::lcu::LcuCredentials;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectPlayer {
    pub cell_id: i64,
    pub champion_id: u32,
    #[serde(default)]
    pub assigned_position: String,
    #[serde(default, skip_deserializing)]
    pub champion_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSession {
    my_team: Vec<ChampSelectPlayer>,
    their_team: Vec<ChampSelectPlayer>,
    local_player_cell_id: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectSession {
    pub my_team: Vec<ChampSelectPlayer>,
    pub their_team: Vec<ChampSelectPlayer>,
    pub local_player_cell_id: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ChampSelectStatus {
    NotInChampSelect,
    InChampSelect { session: ChampSelectSession },
}

/// Consulta a sessão de champion select. Retorna `NotInChampSelect` tanto quando
/// o jogador não está em champ select quanto se a LCU estiver inacessível — o
/// chamador já sabe que está conectado, então qualquer erro aqui só significa
/// "não tem sessão ativa agora".
pub async fn fetch_status(creds: &LcuCredentials) -> ChampSelectStatus {
    let Ok(client) = crate::lcu::build_client() else {
        return ChampSelectStatus::NotInChampSelect;
    };

    let url = format!("{}/lol-champ-select/v1/session", creds.base_url());

    let Ok(response) = client
        .get(url)
        .basic_auth("riot", Some(&creds.token))
        .send()
        .await
    else {
        return ChampSelectStatus::NotInChampSelect;
    };

    if !response.status().is_success() {
        return ChampSelectStatus::NotInChampSelect;
    }

    match response.json::<RawSession>().await {
        Ok(raw) => ChampSelectStatus::InChampSelect {
            session: ChampSelectSession {
                my_team: raw.my_team,
                their_team: raw.their_team,
                local_player_cell_id: raw.local_player_cell_id,
            },
        },
        Err(_) => ChampSelectStatus::NotInChampSelect,
    }
}
