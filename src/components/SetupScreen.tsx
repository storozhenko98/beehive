import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

interface DirEntry {
  name: string;
  path: string;
  is_dir: boolean;
}

interface Props {
  onSetup: (dir: string) => void;
}

export function SetupScreen({ onSetup }: Props) {
  const [dir, setDir] = useState("");
  const [suggestions, setSuggestions] = useState<DirEntry[]>([]);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [selectedIdx, setSelectedIdx] = useState(-1);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Pre-fill with home dir
    invoke<string>("get_home_dir").then((home) => {
      setDir(home + "/beehive");
    });
  }, []);

  const fetchSuggestions = useCallback(async (path: string) => {
    if (!path) {
      setSuggestions([]);
      return;
    }
    try {
      const dirs = await invoke<DirEntry[]>("list_dirs", { path });
      // Filter by typed basename if path doesn't end with /
      const lastSlash = path.lastIndexOf("/");
      const partial = lastSlash >= 0 ? path.slice(lastSlash + 1) : "";

      let filtered = dirs;
      if (partial && !path.endsWith("/")) {
        filtered = dirs.filter((d) =>
          d.name.toLowerCase().startsWith(partial.toLowerCase())
        );
      }

      setSuggestions(filtered);
      setSelectedIdx(-1);
      setShowSuggestions(filtered.length > 0);
    } catch {
      setSuggestions([]);
    }
  }, []);

  useEffect(() => {
    const timer = setTimeout(() => fetchSuggestions(dir), 150);
    return () => clearTimeout(timer);
  }, [dir, fetchSuggestions]);

  function selectSuggestion(entry: DirEntry) {
    setDir(entry.path + "/");
    setShowSuggestions(false);
    inputRef.current?.focus();
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (!showSuggestions) {
      if (e.key === "Enter") {
        handleSetup();
      }
      return;
    }

    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIdx((i) => Math.min(i + 1, suggestions.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIdx((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (selectedIdx >= 0 && selectedIdx < suggestions.length) {
        selectSuggestion(suggestions[selectedIdx]);
      } else {
        setShowSuggestions(false);
        handleSetup();
      }
    } else if (e.key === "Escape") {
      setShowSuggestions(false);
    } else if (e.key === "Tab" && suggestions.length > 0) {
      e.preventDefault();
      const idx = selectedIdx >= 0 ? selectedIdx : 0;
      selectSuggestion(suggestions[idx]);
    }
  }

  async function handleBrowse() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Choose beehive directory",
      });
      if (selected) {
        setDir(selected as string);
        setShowSuggestions(false);
      }
    } catch (e) {
      console.error("Browse failed:", e);
    }
  }

  async function handleSetup() {
    const trimmed = dir.trim().replace(/\/+$/, "");
    if (!trimmed) {
      setError("Please enter a directory path");
      return;
    }
    setLoading(true);
    setError("");
    try {
      await invoke("init_beehive", { dir: trimmed });
      onSetup(trimmed);
    } catch (e) {
      setError(`${e}`);
    }
    setLoading(false);
  }

  // Scroll selected item into view
  useEffect(() => {
    if (selectedIdx >= 0 && dropdownRef.current) {
      const items = dropdownRef.current.querySelectorAll(".suggestion-item");
      items[selectedIdx]?.scrollIntoView({ block: "nearest" });
    }
  }, [selectedIdx]);

  return (
    <div className="screen-center">
      <div className="card" style={{ maxWidth: 520 }}>
        <h1 style={{ marginBottom: 8 }}>&#x2B21; Beehive</h1>
        <p style={{ color: "var(--text-secondary)", marginBottom: 24 }}>
          Choose a directory for your beehive. This is where all your hives
          (repos) and combs (workspaces) will live.
        </p>

        <div className="form-group">
          <label>Beehive directory</label>
          <div className="path-input-group">
            <div className="path-input-wrapper">
              <input
                ref={inputRef}
                type="text"
                value={dir}
                onChange={(e) => {
                  setDir(e.target.value);
                  setError("");
                }}
                onKeyDown={handleKeyDown}
                onFocus={() => dir && fetchSuggestions(dir)}
                onBlur={() => {
                  // Delay to allow click on suggestion
                  setTimeout(() => setShowSuggestions(false), 200);
                }}
                placeholder="/path/to/your/beehive"
                autoFocus
              />
              {showSuggestions && suggestions.length > 0 && (
                <div className="suggestions-dropdown" ref={dropdownRef}>
                  {suggestions.map((entry, i) => (
                    <div
                      key={entry.path}
                      className={`suggestion-item ${i === selectedIdx ? "selected" : ""}`}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        selectSuggestion(entry);
                      }}
                      onMouseEnter={() => setSelectedIdx(i)}
                    >
                      <span className="suggestion-icon">&#128193;</span>
                      <span className="suggestion-name">{entry.name}</span>
                      <span className="suggestion-path">{entry.path}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
            <button className="btn btn-secondary browse-btn" onClick={handleBrowse} type="button">
              Browse
            </button>
          </div>
          <span className="form-hint">
            Directory will be created if it doesn't exist. Tab to autocomplete.
          </span>
        </div>

        {error && <div className="error-box">{error}</div>}

        <button
          className="btn btn-primary"
          onClick={handleSetup}
          disabled={loading}
          style={{ marginTop: 8 }}
        >
          {loading ? "Setting up..." : "Initialize Beehive"}
        </button>
      </div>
    </div>
  );
}
