import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface PreflightResult {
  ok: boolean;
  gitAvailable: boolean;
  ghAvailable: boolean;
  ghAuthenticated: boolean;
  messages: string[];
}

interface Props {
  onPass: () => void;
}

export function PreflightScreen({ onPass }: Props) {
  const [result, setResult] = useState<PreflightResult | null>(null);
  const [checking, setChecking] = useState(true);

  useEffect(() => {
    check();
  }, []);

  async function check() {
    setChecking(true);
    try {
      const r = await invoke<PreflightResult>("preflight_check");
      setResult(r);
      if (r.ok) {
        setTimeout(onPass, 800);
      }
    } catch (e) {
      setResult({
        ok: false,
        gitAvailable: false,
        ghAvailable: false,
        ghAuthenticated: false,
        messages: [`Preflight check failed: ${e}`],
      });
    }
    setChecking(false);
  }

  return (
    <div className="screen-center">
      <div className="card" style={{ maxWidth: 500 }}>
        <h1 style={{ marginBottom: 16 }}>&#x2B21; Beehive</h1>
        <h3 style={{ color: "var(--text-secondary)", marginBottom: 24 }}>
          Checking dependencies...
        </h3>

        {result && (
          <div className="preflight-checks">
            <CheckItem
              label="git"
              ok={result.gitAvailable}
              checking={checking}
            />
            <CheckItem
              label="gh CLI"
              ok={result.ghAvailable}
              checking={checking}
            />
            <CheckItem
              label="gh authenticated"
              ok={result.ghAuthenticated}
              checking={checking}
            />
          </div>
        )}

        {result && !result.ok && (
          <div style={{ marginTop: 20 }}>
            <div className="error-box">
              {result.messages
                .filter((m) => !m.startsWith("git:") && !m.startsWith("gh:"))
                .map((m, i) => (
                  <p key={i}>{m}</p>
                ))}
            </div>
            <button className="btn btn-primary" onClick={check} style={{ marginTop: 12 }}>
              Retry
            </button>
          </div>
        )}

        {result?.ok && (
          <p style={{ color: "var(--success)", marginTop: 16 }}>
            All checks passed
          </p>
        )}
      </div>
    </div>
  );
}

function CheckItem({ label, ok, checking }: { label: string; ok: boolean; checking: boolean }) {
  return (
    <div className="check-item">
      <span className={`check-icon ${ok ? "ok" : "fail"}`}>
        {checking ? "..." : ok ? "OK" : "X"}
      </span>
      <span>{label}</span>
    </div>
  );
}
