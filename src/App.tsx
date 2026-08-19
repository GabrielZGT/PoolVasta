import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

type Summoner = {
  puuid: string;
  displayName: string;
  summonerLevel: number;
  profileIconId: number;
};

type LcuStatus =
  | { state: "disconnected" }
  | { state: "connected"; summoner: Summoner };

type ChampSelectPlayer = {
  cellId: number;
  championId: number;
  assignedPosition: string;
  championName: string | null;
};

type ChampSelectSession = {
  myTeam: ChampSelectPlayer[];
  theirTeam: ChampSelectPlayer[];
  localPlayerCellId: number;
};

type ChampSelectStatus =
  | { state: "notInChampSelect" }
  | { state: "inChampSelect"; session: ChampSelectSession };

type PoolEntry = {
  championId: number;
  championName: string;
  masteryLevel: number;
  masteryPoints: number;
  highestGrade: string | null;
  gamesRecent: number;
  winsRecent: number;
  winRateRecent: number | null;
  daysSinceLastPlayed: number;
  isRusty: boolean;
  score: number;
};

type PlayerIntel = {
  cellId: number;
  lifetimeTopChampionId: number;
  lifetimeTopChampionName: string;
  lifetimeTopChampionPoints: number;
  lifetimeTopChampionLevel: number;
  lifetimeShare: number;
  recentChampionId: number | null;
  recentChampionName: string | null;
  recentGamesAnalyzed: number;
  recentGamesOnTop: number;
  isSpecialist: boolean;
  suggestedBanChampionId: number;
  suggestedBanChampionName: string;
};

type TeamIntel = {
  ally: PlayerIntel[];
  enemy: PlayerIntel[];
};

type SuggestedPick = {
  championId: number;
  championName: string;
  poolScore: number;
  compFitScore: number;
  totalScore: number;
  reasons: string[];
};

function playerLabel(player: ChampSelectPlayer) {
  if (player.championName) return player.championName;
  if (player.championId !== 0) return `Campeão #${player.championId}`;
  return "—";
}

function PlayerIntelBadge({ intel }: { intel: PlayerIntel | undefined }) {
  if (!intel) return null;

  if (intel.isSpecialist) {
    const recentNote =
      intel.recentGamesAnalyzed > 0
        ? `${intel.recentGamesOnTop}/${intel.recentGamesAnalyzed} partidas recentes`
        : `${Math.round(intel.lifetimeShare * 100)}% da maestria vitalícia`;
    return (
      <span className="badge onetrick">
        Sugestão: banir {intel.suggestedBanChampionName} · {recentNote}
      </span>
    );
  }

  return (
    <span className="badge">
      Principal: {intel.lifetimeTopChampionName} ({Math.round(intel.lifetimeShare * 100)}%)
    </span>
  );
}

function App() {
  const [lcuStatus, setLcuStatus] = useState<LcuStatus>({ state: "disconnected" });
  const [champSelect, setChampSelect] = useState<ChampSelectStatus>({
    state: "notInChampSelect",
  });
  const [pool, setPool] = useState<PoolEntry[]>([]);
  const [poolLoading, setPoolLoading] = useState(false);
  const [poolError, setPoolError] = useState<string | null>(null);

  const [hasApiKey, setHasApiKey] = useState(false);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [savingKey, setSavingKey] = useState(false);

  const [intel, setIntel] = useState<TeamIntel | null>(null);
  const [intelLoading, setIntelLoading] = useState(false);
  const [intelError, setIntelError] = useState<string | null>(null);

  const [suggestions, setSuggestions] = useState<SuggestedPick[]>([]);

  useEffect(() => {
    invoke<LcuStatus>("get_lcu_status").then(setLcuStatus);
    invoke<boolean>("has_riot_api_key").then(setHasApiKey);

    const unlistenLcu = listen<LcuStatus>("lcu-status", (event) => {
      setLcuStatus(event.payload);
    });
    const unlistenChampSelect = listen<ChampSelectStatus>("champ-select-status", (event) => {
      setChampSelect(event.payload);
    });
    const unlistenSuggestions = listen<SuggestedPick[]>("champ-select-suggestions", (event) => {
      setSuggestions(event.payload);
    });

    return () => {
      unlistenLcu.then((fn) => fn());
      unlistenChampSelect.then((fn) => fn());
      unlistenSuggestions.then((fn) => fn());
    };
  }, []);

  const connected = lcuStatus.state === "connected";

  useEffect(() => {
    if (!connected) return;
    loadPool();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connected]);

  useEffect(() => {
    if (champSelect.state === "notInChampSelect") {
      setIntel(null);
      setIntelError(null);
    }
  }, [champSelect.state]);

  async function loadPool() {
    setPoolLoading(true);
    setPoolError(null);
    try {
      const result = await invoke<PoolEntry[]>("get_champion_pool");
      setPool(result);
    } catch (err) {
      setPoolError(String(err));
    } finally {
      setPoolLoading(false);
    }
  }

  async function saveApiKey() {
    if (!apiKeyInput.trim()) return;
    setSavingKey(true);
    try {
      await invoke("set_riot_api_key", { key: apiKeyInput.trim() });
      setHasApiKey(true);
      setApiKeyInput("");
    } finally {
      setSavingKey(false);
    }
  }

  async function analyzeTeam() {
    setIntelLoading(true);
    setIntelError(null);
    try {
      const result = await invoke<TeamIntel>("get_champ_select_intel");
      setIntel(result);
    } catch (err) {
      setIntelError(String(err));
    } finally {
      setIntelLoading(false);
    }
  }

  return (
    <main className="container">
      <h1>PoolVasta</h1>

      <div className="status-row">
        <span className={`status-dot ${connected ? "online" : "offline"}`} />
        <span>
          {connected
            ? `Conectado como ${lcuStatus.summoner.displayName} (nível ${lcuStatus.summoner.summonerLevel})`
            : "Cliente do League não encontrado"}
        </span>
      </div>

      {!connected && (
        <p className="hint">Abra o cliente do League of Legends pra conectar automaticamente.</p>
      )}

      {connected && (
        <section className="panel">
          <h2>Riot API key</h2>
          <p className="hint">
            Usada só pra buscar maestria de aliados/inimigos em champion select (a LCU não dá
            acesso a dados de outros jogadores). Chave pessoal expira em 24h — gere uma nova em{" "}
            developer.riotgames.com quando parar de funcionar.
          </p>
          {hasApiKey ? (
            <p className="hint">✓ Chave configurada.</p>
          ) : null}
          <div className="key-row">
            <input
              type="password"
              placeholder={hasApiKey ? "Colar nova chave..." : "Colar sua Riot API key..."}
              value={apiKeyInput}
              onChange={(e) => setApiKeyInput(e.currentTarget.value)}
            />
            <button onClick={saveApiKey} disabled={savingKey || !apiKeyInput.trim()}>
              {savingKey ? "Salvando..." : "Salvar"}
            </button>
          </div>
        </section>
      )}

      {connected && champSelect.state === "inChampSelect" && (
        <section className="panel">
          <div className="panel-header">
            <h2>Champion Select</h2>
            <button onClick={analyzeTeam} disabled={intelLoading || !hasApiKey}>
              {intelLoading ? "Analisando..." : "Analisar time"}
            </button>
          </div>
          <div className="teams">
            <div className="team">
              <h3>Seu time</h3>
              <ul>
                {champSelect.session.myTeam.map((player) => (
                  <li
                    key={player.cellId}
                    className={player.cellId === champSelect.session.localPlayerCellId ? "me" : ""}
                  >
                    {playerLabel(player)}
                    {player.assignedPosition && (
                      <span className="position"> · {player.assignedPosition}</span>
                    )}
                    <PlayerIntelBadge intel={intel?.ally.find((i) => i.cellId === player.cellId)} />
                  </li>
                ))}
              </ul>
            </div>
            <div className="team">
              <h3>Time inimigo</h3>
              <ul>
                {champSelect.session.theirTeam.map((player) => (
                  <li key={player.cellId}>
                    {playerLabel(player)}
                    <PlayerIntelBadge intel={intel?.enemy.find((i) => i.cellId === player.cellId)} />
                  </li>
                ))}
              </ul>
            </div>
          </div>

          {intelError && <p className="error">{intelError}</p>}
          {!intelError && intel && intel.enemy.length === 0 && (
            <p className="hint">
              Nenhum inimigo com maestria visível (identidade oculta pela Riot — comum em
              ranqueada).
            </p>
          )}

          {suggestions.length > 0 && (
            <div className="suggestions">
              <h3>Sugestão de pick</h3>
              <p className="hint">
                Automático, a partir do seu pool + lacunas óbvias do time (falta tank, time todo
                do mesmo tipo de dano). Não é dado de contra-pick real, só heurística.
              </p>
              <ul className="pool-list">
                {suggestions.map((s) => (
                  <li key={s.championId} className="pool-entry">
                    <div className="pool-entry-main">
                      <span className="pool-champion-name">{s.championName}</span>
                      <span className="pool-mastery">score {Math.round(s.totalScore * 100)}</span>
                    </div>
                    <div className="pool-entry-meta">
                      {s.reasons.map((reason) => (
                        <span className="badge" key={reason}>
                          {reason}
                        </span>
                      ))}
                    </div>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </section>
      )}

      {connected && (
        <section className="panel">
          <div className="panel-header">
            <h2>Meu pool</h2>
            <button onClick={loadPool} disabled={poolLoading}>
              {poolLoading ? "Carregando..." : "Atualizar"}
            </button>
          </div>
          <p className="hint">
            Nota mostrada é a melhor da temporada com o campeão (a LCU não guarda nota por
            partida nem uma média real). Win rate é sobre suas últimas 20 partidas gerais.
          </p>

          {poolError && <p className="error">{poolError}</p>}

          <ul className="pool-list">
            {pool.map((entry) => (
              <li key={entry.championId} className="pool-entry">
                <div className="pool-entry-main">
                  <span className="pool-champion-name">{entry.championName}</span>
                  <span className="pool-mastery">
                    Nível {entry.masteryLevel} · {entry.masteryPoints.toLocaleString("pt-BR")} pts
                  </span>
                </div>
                <div className="pool-entry-meta">
                  {entry.highestGrade && <span className="badge grade">{entry.highestGrade}</span>}
                  <span className="badge">
                    {entry.gamesRecent > 0
                      ? `${entry.winsRecent}V-${entry.gamesRecent - entry.winsRecent}D em ${entry.gamesRecent} recentes`
                      : "sem jogos recentes"}
                  </span>
                  {entry.isRusty && (
                    <span className="badge rusty">
                      Pode estar enferrujado — {entry.daysSinceLastPlayed}d sem jogar
                    </span>
                  )}
                </div>
              </li>
            ))}
          </ul>

          {!poolLoading && pool.length === 0 && !poolError && (
            <p className="hint">Nenhum campeão passou nos critérios mínimos de pool ainda.</p>
          )}
        </section>
      )}
    </main>
  );
}

export default App;
