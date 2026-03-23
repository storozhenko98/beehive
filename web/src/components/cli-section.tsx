import { CopyButton } from "./copy-button";

const INSTALL_CMD = "curl -fsSL beehiveapp.dev/install.sh | bash";

export function CliSection() {
  return (
    <section className="py-20" id="cli">
      <p className="text-center text-xs font-semibold tracking-[2px] uppercase text-ctp-blue mb-3">
        CLI / TUI
      </p>
      <h2 className="text-center text-4xl font-bold tracking-tight mb-12">
        Prefer the terminal?
      </h2>
      <div className="bg-ctp-base border border-ctp-surface0 rounded-2xl p-12 px-10 max-w-[700px] mx-auto max-md:p-8 max-md:px-6">
        <h3 className="text-xl font-bold mb-2">Beehive TUI</h3>
        <p className="text-sm text-ctp-subtext0 leading-relaxed mb-5">
          A terminal-native interface with the same workspace management,
          embedded terminals, and shared config as the desktop app. Supported
          on macOS and Linux x64. Install with one command, or grab the latest
          binary from Releases. You&apos;ll be asked to choose{" "}
          <code className="font-mono text-ctp-text">bh</code> or{" "}
          <code className="font-mono text-ctp-text">beehive</code> as your
          command name.
        </p>
        <div className="bg-ctp-mantle border border-ctp-surface0 rounded-lg p-3.5 px-4 overflow-x-auto">
          <code className="text-[13px] text-ctp-green whitespace-nowrap font-mono">
            {INSTALL_CMD}
          </code>
        </div>
        <CopyButton text={INSTALL_CMD} />
        <p className="mt-3 text-[13px] text-ctp-overlay0">
          Linux x64 release binary:{" "}
          <a
            href="https://github.com/storozhenko98/beehive/releases/latest"
            className="text-ctp-blue hover:text-ctp-sapphire underline underline-offset-4"
          >
            download from Releases
          </a>
        </p>
        <div className="flex gap-6 mt-6 flex-wrap">
          <div className="flex-1 min-w-[160px] text-[13px] text-ctp-subtext0">
            <strong className="text-ctp-text block mb-0.5 text-[13px]">
              Same config
            </strong>
            Shares ~/.beehive with the GUI app
          </div>
          <div className="flex-1 min-w-[160px] text-[13px] text-ctp-subtext0">
            <strong className="text-ctp-text block mb-0.5 text-[13px]">
              Embedded PTY
            </strong>
            Full terminal emulator inside the TUI
          </div>
          <div className="flex-1 min-w-[160px] text-[13px] text-ctp-subtext0">
            <strong className="text-ctp-text block mb-0.5 text-[13px]">
              Auto-update
            </strong>
            Checks for updates on startup
          </div>
        </div>
      </div>
    </section>
  );
}
