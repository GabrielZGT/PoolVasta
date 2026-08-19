use serde::{Deserialize, Serialize};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

pub struct LcuCredentials {
    pub port: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summoner {
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "summonerLevel")]
    pub summoner_level: u32,
    #[serde(rename = "profileIconId")]
    pub profile_icon_id: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum LcuStatus {
    Disconnected,
    Connected { summoner: Summoner },
}

/// Procura o processo do cliente do League e extrai porta/token da LCU API
/// direto da linha de comando dele (mesma abordagem usada por league-connect e lcu-driver).
pub fn find_credentials(system: &mut System) -> Option<LcuCredentials> {
    // sysinfo não busca a linha de comando por padrão (custo de performance) — precisa pedir explicitamente.
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );

    system
        .processes()
        .values()
        .find(|process| {
            process
                .name()
                .to_string_lossy()
                .eq_ignore_ascii_case("LeagueClientUx.exe")
        })
        .and_then(|process| {
            let cmd: Vec<String> = process
                .cmd()
                .iter()
                .map(|arg| arg.to_string_lossy().to_string())
                .collect();

            let port = extract_arg(&cmd, "--app-port=")?;
            let token = extract_arg(&cmd, "--remoting-auth-token=")?;
            Some(LcuCredentials { port, token })
        })
}

fn extract_arg(cmd: &[String], prefix: &str) -> Option<String> {
    cmd.iter()
        .find_map(|arg| arg.strip_prefix(prefix).map(str::to_string))
}

pub async fn fetch_current_summoner(creds: &LcuCredentials) -> Result<Summoner, String> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true) // LCU usa certificado autoassinado, só em 127.0.0.1
        .build()
        .map_err(|err| err.to_string())?;

    let url = format!(
        "https://127.0.0.1:{}/lol-summoner/v1/current-summoner",
        creds.port
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

    response.json::<Summoner>().await.map_err(|err| err.to_string())
}

pub async fn current_status(system: &mut System) -> LcuStatus {
    match find_credentials(system) {
        Some(creds) => match fetch_current_summoner(&creds).await {
            Ok(summoner) => LcuStatus::Connected { summoner },
            Err(_) => LcuStatus::Disconnected,
        },
        None => LcuStatus::Disconnected,
    }
}
