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

function playerLabel(player: ChampSelectPlayer) {
  if (player.championName) return player.championName;
  if (player.championId !== 0) return `Campeão #${player.championId}`;
  return "—";
}

function App() {
  const [lcuStatus, setLcuStatus] = useState<LcuStatus>({ state: "disconnected" });
  const [champSelect, setChampSelect] = useState<ChampSelectStatus>({
    state: "notInChampSelect",
  });
  const [pool, setPool] = useState<PoolEntry[]>([]);
  const [poolLoading, setPoolLoading] = useState(false);
  const [poolError, setPoolError] = useState<string | null>(null);

  useEffect(() => {
    invoke<LcuStatus>("get_lcu_status").then(setLcuStatus);

    const unlistenLcu = listen<LcuStatus>("lcu-status", (event) => {
      setLcuStatus(event.payload);
    });
    const unlistenChampSelect = listen<ChampSelectStatus>("champ-select-status", (event) => {
      setChampSelect(event.payload);
    });

    return () => {
      unlistenLcu.then((fn) => fn());
      unlistenChampSelect.then((fn) => fn());
    };
  }, []);

  const connected = lcuStatus.state === "connected";

  useEffect(() => {
    if (!connected) return;
    loadPool();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connected]);

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

      {connected && champSelect.state === "inChampSelect" && (
        <section className="panel">
          <h2>Champion Select</h2>
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
                  </li>
                ))}
              </ul>
            </div>
            <div className="team">
              <h3>Time inimigo</h3>
              <ul>
                {champSelect.session.theirTeam.map((player) => (
                  <li key={player.cellId}>{playerLabel(player)}</li>
                ))}
              </ul>
            </div>
          </div>
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
