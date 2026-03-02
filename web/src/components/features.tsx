const features = [
  {
    icon: "\u2B21",
    color: "bg-ctp-blue/10 text-ctp-blue",
    title: "Multi-repo management",
    description:
      "Add GitHub repos and switch between them instantly. All your projects in one window.",
  },
  {
    icon: "\u2442",
    color: "bg-ctp-teal/10 text-ctp-teal",
    title: "Isolated workspaces",
    description:
      "Full git clones on any branch, each in its own directory. No interference between experiments.",
  },
  {
    icon: "\u2588",
    color: "bg-ctp-mauve/10 text-ctp-mauve",
    title: "Persistent terminals",
    description:
      "Real PTY sessions that stay alive across workspace and repo switches. No lost state.",
  },
  {
    icon: "\u276F",
    color: "bg-ctp-peach/10 text-ctp-peach",
    title: "Agent panes",
    description:
      "Launch Claude Code or any CLI agent alongside your terminals in a flexible grid.",
  },
  {
    icon: "\u2398",
    color: "bg-ctp-green/10 text-ctp-green",
    title: "Copy workspaces",
    description:
      "Duplicate a workspace including uncommitted work. Experiment without risk.",
  },
  {
    icon: "\u21C4",
    color: "bg-ctp-sapphire/10 text-ctp-sapphire",
    title: "Live branch tracking",
    description:
      "Sidebar updates when you switch branches in the terminal. Always know where you are.",
  },
];

export function Features() {
  return (
    <section className="py-20" id="features">
      <p className="text-center text-xs font-semibold tracking-[2px] uppercase text-ctp-blue mb-3">
        Features
      </p>
      <h2 className="text-center text-4xl font-bold tracking-tight mb-12">
        Everything you need to work with agents
      </h2>
      <div className="grid grid-cols-3 gap-4 max-md:grid-cols-1">
        {features.map((f) => (
          <div
            key={f.title}
            className="bg-ctp-base border border-ctp-surface0 rounded-2xl p-7 transition-all hover:border-ctp-surface1 hover:-translate-y-0.5 hover:shadow-[0_8px_32px_rgba(0,0,0,0.2)]"
          >
            <div
              className={`w-10 h-10 rounded-xl flex items-center justify-center text-lg mb-4 ${f.color}`}
            >
              {f.icon}
            </div>
            <h3 className="text-[15px] font-semibold mb-2">{f.title}</h3>
            <p className="text-[13px] text-ctp-subtext0 leading-relaxed">
              {f.description}
            </p>
          </div>
        ))}
      </div>
    </section>
  );
}
