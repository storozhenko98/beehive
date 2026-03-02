export function Mockup() {
  return (
    <section className="py-5 pb-20">
      <div className="bg-ctp-base border border-ctp-surface0 rounded-xl overflow-hidden shadow-[0_24px_80px_rgba(0,0,0,0.5),0_0_0_1px_rgba(137,180,250,0.05)] max-w-[820px] mx-auto">
        {/* Title bar */}
        <div className="flex items-center gap-2 px-4 py-2.5 bg-ctp-mantle border-b border-ctp-surface0">
          <div className="w-3 h-3 rounded-full bg-ctp-pink" />
          <div className="w-3 h-3 rounded-full bg-ctp-yellow" />
          <div className="w-3 h-3 rounded-full bg-ctp-green" />
          <span className="flex-1 text-center text-xs text-ctp-overlay0 font-medium">
            Beehive
          </span>
          <div className="w-9" />
        </div>

        {/* Body */}
        <div className="flex h-[380px] max-md:h-[300px]">
          {/* Sidebar */}
          <div className="w-[200px] min-w-[200px] max-md:w-[150px] max-md:min-w-[150px] bg-ctp-mantle border-r border-ctp-surface0 flex flex-col text-xs">
            <div className="px-3.5 py-2.5 border-b border-ctp-surface0 text-sm font-bold text-ctp-text">
              &#x2B21; Beehive
            </div>
            <div className="px-3.5 pt-1.5 pb-0.5 text-[9px] font-semibold tracking-wide uppercase text-ctp-overlay0">
              Hive
            </div>
            <div className="mx-2.5 my-1 px-2 py-1.5 rounded-md border border-ctp-surface0 bg-ctp-surface0 text-ctp-text text-[11px] flex justify-between items-center">
              <span>my-saas-app</span>
              <span className="text-[8px] text-ctp-overlay0">&#x25BC;</span>
            </div>
            <div className="px-3.5 pt-1.5 pb-0.5 text-[9px] font-semibold tracking-wide uppercase text-ctp-overlay0">
              Combs
            </div>
            <div className="flex-1 overflow-hidden py-0.5">
              <CombItem name="feature-auth" branch="feat/oauth" active />
              <CombItem name="fix-payments" branch="main" />
              <CombItem name="refactor-api" branch="dev" />
              <CombItem name="experiment-v2" branch="experiment" />
            </div>
            <div className="mx-2.5 mb-1.5 py-1.5 rounded-md border border-dashed border-ctp-surface0 text-center text-[11px] text-ctp-subtext0">
              + New Comb
            </div>
            <div className="border-t border-ctp-surface0 px-2.5 py-1.5">
              <div className="py-1 px-1.5 text-[11px] text-ctp-overlay0 rounded-sm">
                Manage Hives
              </div>
              <div className="py-1 px-1.5 text-[11px] text-ctp-overlay0 rounded-sm">
                Settings
              </div>
            </div>
          </div>

          {/* Workspace */}
          <div className="flex-1 flex flex-col overflow-hidden">
            <div className="flex items-center justify-between px-3.5 py-1.5 bg-ctp-mantle border-b border-ctp-surface0 min-h-9">
              <div className="flex items-center gap-2">
                <strong className="text-[13px]">feature-auth</strong>
                <span className="text-[10px] font-mono text-ctp-overlay0 bg-ctp-surface0 px-1.5 py-px rounded-sm">
                  feat/oauth
                </span>
              </div>
              <span className="text-[11px] px-2.5 py-1 rounded-md border border-ctp-surface0 bg-ctp-surface0 text-ctp-subtext0">
                + Terminal
              </span>
            </div>
            <div className="flex-1 grid grid-cols-2 max-md:grid-cols-1 gap-px bg-ctp-surface0 overflow-hidden">
              {/* Terminal pane */}
              <div className="bg-ctp-base flex flex-col">
                <div className="flex items-center justify-between px-2 py-px bg-ctp-mantle border-b border-ctp-surface0 min-h-5">
                  <span className="text-[8px] font-bold tracking-wide px-1 py-px rounded-sm bg-ctp-surface0 text-ctp-subtext0">
                    TERM
                  </span>
                  <span className="text-[10px] text-ctp-overlay0 font-mono">
                    x
                  </span>
                </div>
                <div className="flex-1 p-2 px-2.5 font-mono text-[10px] leading-[1.7] text-ctp-subtext0 overflow-hidden">
                  <span className="text-ctp-green">~</span>{" "}
                  <span className="text-ctp-text">git status</span>
                  <br />
                  <span className="text-ctp-overlay0">
                    On branch feat/oauth
                  </span>
                  <br />
                  <span className="text-ctp-overlay0">
                    Changes not staged:
                  </span>
                  <br />
                  <span className="text-ctp-overlay0">
                    &nbsp;&nbsp;modified: src/auth.ts
                  </span>
                  <br />
                  <span className="text-ctp-overlay0">
                    &nbsp;&nbsp;modified: src/middleware.ts
                  </span>
                  <br />
                  <br />
                  <span className="text-ctp-green">~</span>{" "}
                  <span className="text-ctp-text">npm test</span>
                  <br />
                  <span className="text-ctp-green">PASS</span>{" "}
                  <span className="text-ctp-overlay0">
                    src/auth.test.ts (12 tests)
                  </span>
                  <br />
                  <span className="text-ctp-green">PASS</span>{" "}
                  <span className="text-ctp-overlay0">
                    src/middleware.test.ts (8 tests)
                  </span>
                  <br />
                  <br />
                  <span className="text-ctp-green">~</span>{" "}
                  <span className="inline-block w-[7px] h-3 bg-ctp-blue align-text-bottom animate-[blink_1.2s_step-end_infinite]" />
                </div>
              </div>

              {/* Agent pane */}
              <div className="bg-ctp-base flex flex-col">
                <div className="flex items-center justify-between px-2 py-px bg-ctp-mantle border-b border-ctp-surface0 min-h-5">
                  <span className="text-[8px] font-bold tracking-wide px-1 py-px rounded-sm bg-ctp-surface0 text-ctp-subtext0">
                    AGENT
                  </span>
                  <span className="text-[10px] text-ctp-overlay0 font-mono">
                    x
                  </span>
                </div>
                <div className="flex-1 p-2 px-2.5 font-mono text-[10px] leading-[1.7] text-ctp-subtext0 overflow-hidden">
                  {/* Claude Code welcome block — clean CSS left border instead of box-drawing chars */}
                  <div className="border-l-2 border-ctp-blue/40 pl-2.5 mb-2">
                    <span className="text-ctp-blue font-semibold">Claude Code</span>
                    <br />
                    <br />
                    <span className="text-ctp-text font-semibold">Welcome back!</span>
                    <br />
                    <br />
                    <span className="text-ctp-overlay0">Opus 4.6</span>
                    <br />
                    <span className="text-ctp-overlay0">~/feature-auth</span>
                  </div>
                  <br />
                  <span className="text-ctp-blue">&#x276F;</span>{" "}
                  <span className="text-ctp-overlay0">
                    add OAuth2 flow to
                  </span>
                  <br />
                  <span className="text-ctp-overlay0">
                    &nbsp;&nbsp;src/auth.ts
                  </span>
                  <br />
                  <br />
                  <span className="text-ctp-blue">&#x276F;</span>{" "}
                  <span className="inline-block w-[7px] h-3 bg-ctp-blue align-text-bottom animate-[blink_1.2s_step-end_infinite]" />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function CombItem({
  name,
  branch,
  active,
}: {
  name: string;
  branch: string;
  active?: boolean;
}) {
  return (
    <div
      className={`py-1.5 px-2.5 pl-3.5 border-l-[3px] ${
        active
          ? "bg-ctp-surface0 border-l-ctp-blue"
          : "border-l-transparent"
      }`}
    >
      <div className="text-xs font-medium text-ctp-text">{name}</div>
      <div className="text-[9px] font-mono text-ctp-overlay0">{branch}</div>
    </div>
  );
}
