"use client";

import { useState } from "react";

export function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };

  return (
    <button
      onClick={handleCopy}
      className="mt-3 inline-flex items-center justify-center bg-ctp-surface0 border border-ctp-surface0 rounded-md text-ctp-subtext0 text-[13px] px-5 py-2 cursor-pointer font-sans hover:bg-ctp-surface1 hover:text-ctp-text transition-all"
    >
      {copied ? "Copied!" : "Copy"}
    </button>
  );
}

export function CopyBar({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };

  return (
    <div
      onClick={handleCopy}
      className="flex items-center gap-2.5 max-w-[620px] mx-auto mb-6 bg-ctp-base border border-ctp-surface0 rounded-xl px-4 py-3.5 cursor-pointer relative hover:border-ctp-surface1 transition-colors"
    >
      <span className="font-mono text-sm text-ctp-overlay0 select-none shrink-0">
        $
      </span>
      <code className="text-[13px] text-ctp-green whitespace-nowrap overflow-hidden text-ellipsis flex-1 text-left font-mono">
        {text}
      </code>
      <span
        className={`text-xs font-medium select-none px-2.5 py-0.5 rounded-md transition-all ${
          copied
            ? "text-ctp-green bg-ctp-base"
            : "text-ctp-overlay0 bg-ctp-surface0"
        }`}
      >
        {copied ? "Copied!" : "Copy"}
      </span>
    </div>
  );
}
