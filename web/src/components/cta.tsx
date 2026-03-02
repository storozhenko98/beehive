export function Cta() {
  return (
    <section className="py-20 text-center">
      <div className="bg-gradient-to-br from-ctp-blue/5 to-ctp-mauve/5 border border-ctp-surface0 rounded-[20px] py-16 px-10">
        <h2 className="text-[32px] font-bold tracking-tight mb-3">
          Ready to try Beehive?
        </h2>
        <p className="text-base text-ctp-subtext0 mb-8">
          Desktop app or terminal &mdash; same tool, same config. Free and open
          source.
        </p>
        <div className="flex gap-3 justify-center flex-wrap">
          <a
            href="https://github.com/storozhenko98/beehive/releases/latest"
            className="inline-flex items-center gap-2 px-7 py-3.5 rounded-xl text-[15px] font-semibold bg-ctp-blue text-ctp-crust shadow-[0_0_20px_rgba(137,180,250,0.2),inset_0_1px_0_rgba(255,255,255,0.1)] hover:bg-ctp-sapphire hover:shadow-[0_0_30px_rgba(137,180,250,0.3)] hover:-translate-y-px transition-all"
          >
            Download Desktop App
          </a>
          <a
            href="#cli"
            className="inline-flex items-center gap-2 px-7 py-3.5 rounded-xl text-[15px] font-semibold bg-ctp-surface0/50 text-ctp-text border border-ctp-surface0 hover:bg-ctp-surface0 hover:border-ctp-surface1 hover:-translate-y-px transition-all"
          >
            Install TUI
          </a>
          <a
            href="https://github.com/storozhenko98/beehive"
            className="inline-flex items-center gap-2 px-7 py-3.5 rounded-xl text-[15px] font-semibold bg-ctp-surface0/50 text-ctp-text border border-ctp-surface0 hover:bg-ctp-surface0 hover:border-ctp-surface1 hover:-translate-y-px transition-all"
          >
            Star on GitHub
          </a>
        </div>
      </div>
    </section>
  );
}
