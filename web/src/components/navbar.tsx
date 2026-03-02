import Image from "next/image";
import Link from "next/link";

export function Navbar() {
  return (
    <nav className="fixed top-0 left-0 right-0 z-50 bg-ctp-crust/80 backdrop-blur-xl border-b border-ctp-surface0/50">
      <div className="max-w-[960px] mx-auto px-8 h-14 flex items-center justify-between">
        <Link href="#" className="flex items-center gap-2.5 font-bold text-[15px] text-ctp-text">
          <Image src="/logo.png" alt="Beehive" width={28} height={28} className="rounded-md" />
          Beehive
        </Link>
        <div className="flex items-center gap-6">
          <Link href="#features" className="text-[13px] font-medium text-ctp-overlay0 hover:text-ctp-text transition-colors hidden sm:inline">
            Features
          </Link>
          <Link href="#how-it-works" className="text-[13px] font-medium text-ctp-overlay0 hover:text-ctp-text transition-colors hidden sm:inline">
            How it works
          </Link>
          <Link href="#cli" className="text-[13px] font-medium text-ctp-overlay0 hover:text-ctp-text transition-colors hidden sm:inline">
            CLI
          </Link>
          <a href="https://github.com/storozhenko98/beehive" className="text-[13px] font-medium text-ctp-overlay0 hover:text-ctp-text transition-colors hidden sm:inline">
            GitHub
          </a>
          <a
            href="https://github.com/storozhenko98/beehive/releases/latest"
            className="bg-ctp-blue text-ctp-crust px-4 py-1.5 rounded-md font-semibold text-[13px] hover:bg-ctp-sapphire transition-colors"
          >
            Download
          </a>
        </div>
      </div>
    </nav>
  );
}
