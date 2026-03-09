import Image from "next/image";
import { CopyBar } from "./copy-button";

const INSTALL_CMD = "curl -fsSL beehiveapp.dev/install.sh | bash";

export function Hero() {
  return (
    <section className="text-center pt-36 pb-16 relative">
      {/* Background glow */}
      <div className="absolute top-16 left-1/2 -translate-x-1/2 w-[600px] h-[600px] bg-[radial-gradient(circle,rgba(137,180,250,0.08)_0%,rgba(137,180,250,0.02)_40%,transparent_70%)] pointer-events-none" />

      {/* Logo */}
      <div className="relative inline-block mb-8">
        <div className="absolute -inset-5 rounded-[40px] bg-[radial-gradient(circle,rgba(137,180,250,0.15),transparent_70%)] animate-[pulse-glow_4s_ease-in-out_infinite]" />
        <Image
          src="/logo.png"
          alt="Beehive logo"
          width={120}
          height={120}
          priority
          className="relative z-10 rounded-[28px] drop-shadow-[0_0_40px_rgba(137,180,250,0.3)]"
        />
      </div>

      <h1 className="text-[56px] font-extrabold tracking-[-2px] mb-4 bg-gradient-to-br from-ctp-text to-ctp-blue bg-clip-text text-transparent max-md:text-4xl">
        Beehive
      </h1>

      <p className="text-xl text-ctp-subtext0 max-w-[520px] mx-auto mb-10 leading-relaxed max-md:text-base">
        Manage repos, create isolated workspaces, and run coding agents
        side-by-side &mdash; all from one window.
      </p>

      {/* Action buttons */}
      <div className="flex gap-3 justify-center flex-wrap mb-4">
        <a
          href="https://github.com/storozhenko98/beehive/releases/latest"
          className="inline-flex items-center gap-2 px-7 py-3.5 rounded-xl text-[15px] font-semibold bg-ctp-blue text-ctp-crust shadow-[0_0_20px_rgba(137,180,250,0.2),inset_0_1px_0_rgba(255,255,255,0.1)] hover:bg-ctp-sapphire hover:shadow-[0_0_30px_rgba(137,180,250,0.3)] hover:-translate-y-px transition-all"
        >
          Download Desktop App
        </a>
        <a
          href="https://github.com/storozhenko98/beehive"
          className="inline-flex items-center gap-2 px-7 py-3.5 rounded-xl text-[15px] font-semibold bg-ctp-surface0/50 text-ctp-text border border-ctp-surface0 hover:bg-ctp-surface0 hover:border-ctp-surface1 hover:-translate-y-px transition-all"
        >
          View Source
        </a>
      </div>

      {/* Divider */}
      <div className="flex items-center gap-4 max-w-[560px] mx-auto mt-6 mb-4">
        <div className="flex-1 h-px bg-ctp-surface0" />
        <span className="text-[13px] text-ctp-overlay0 font-medium whitespace-nowrap">
          or install the TUI
        </span>
        <div className="flex-1 h-px bg-ctp-surface0" />
      </div>

      {/* Terminal install bar (client component) */}
      <CopyBar text={INSTALL_CMD} />

      {/* Meta info */}
      <p className="text-[13px] text-ctp-overlay0">
        <span>macOS (Apple Silicon) + Linux x64</span>
        <span className="before:content-['·'] before:mx-2 before:text-ctp-surface1">
          Signed &amp; notarized on macOS
        </span>
        <span className="before:content-['·'] before:mx-2 before:text-ctp-surface1">
          GUI + TUI share config
        </span>
        <span className="before:content-['·'] before:mx-2 before:text-ctp-surface1">
          MIT License
        </span>
      </p>
    </section>
  );
}
