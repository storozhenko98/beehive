export function Footer() {
  return (
    <footer className="border-t border-ctp-surface0 py-10">
      <div className="max-w-[960px] mx-auto px-8">
        <div className="flex items-center justify-between max-md:flex-col max-md:gap-4 max-md:text-center">
          <span className="text-[13px] text-ctp-overlay0">
            &copy; 2026{" "}
            <a
              href="https://storozh.dev"
              className="text-ctp-blue hover:text-ctp-sapphire transition-colors"
            >
              Mykyta Storozhenko
            </a>
          </span>
          <div className="flex gap-5">
            <a
              href="https://github.com/storozhenko98/beehive"
              className="text-[13px] text-ctp-overlay0 hover:text-ctp-text transition-colors"
            >
              GitHub
            </a>
            <a
              href="https://github.com/storozhenko98/beehive/releases"
              className="text-[13px] text-ctp-overlay0 hover:text-ctp-text transition-colors"
            >
              Releases
            </a>
            <a
              href="https://github.com/storozhenko98/beehive/issues"
              className="text-[13px] text-ctp-overlay0 hover:text-ctp-text transition-colors"
            >
              Issues
            </a>
            <a
              href="https://x.com/technoleviathan"
              className="text-[13px] text-ctp-overlay0 hover:text-ctp-text transition-colors"
            >
              @technoleviathan
            </a>
          </div>
        </div>
      </div>
    </footer>
  );
}
