import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

type Summoner = {
  displayName: string;
  summonerLevel: number;
  profileIconId: number;
};

type LcuStatus =
  | { state: "disconnected" }
  | { state: "connected"; summoner: Summoner };

function App() {
  const [status, setStatus] = useState<LcuStatus>({ state: "disconnected" });

  useEffect(() => {
    invoke<LcuStatus>("get_lcu_status").then(setStatus);

    const unlisten = listen<LcuStatus>("lcu-status", (event) => {
      setStatus(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const connected = status.state === "connected";

  return (
    <main className="container">
      <h1>PoolVasta</h1>

      <div className="status-row">
        <span className={`status-dot ${connected ? "online" : "offline"}`} />
        <span>
          {connected
            ? `Conectado como ${status.summoner.displayName} (nível ${status.summoner.summonerLevel})`
            : "Cliente do League não encontrado"}
        </span>
      </div>

      {!connected && (
        <p className="hint">
          Abra o cliente do League of Legends pra conectar automaticamente.
        </p>
      )}
    </main>
  );
}

export default App;
