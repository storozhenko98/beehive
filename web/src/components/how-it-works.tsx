const steps = [
  {
    number: 1,
    title: "Add a Hive",
    description:
      "Link a GitHub repo by URL. Beehive stores the reference and metadata.",
  },
  {
    number: 2,
    title: "Create a Comb",
    description:
      "Clone the repo to an isolated directory on any branch you choose.",
  },
  {
    number: 3,
    title: "Open Panes",
    description:
      "Launch terminals and agents side-by-side. Work on multiple tasks at once.",
  },
];

export function HowItWorks() {
  return (
    <section className="py-20" id="how-it-works">
      <p className="text-center text-xs font-semibold tracking-[2px] uppercase text-ctp-blue mb-3">
        How it works
      </p>
      <h2 className="text-center text-4xl font-bold tracking-tight mb-12">
        Three concepts, zero complexity
      </h2>
      <div className="flex gap-6 max-w-[800px] mx-auto max-md:flex-col">
        {steps.map((step, i) => (
          <div
            key={step.number}
            className="flex-1 text-center p-8 px-5 bg-ctp-base border border-ctp-surface0 rounded-2xl relative"
          >
            <div className="w-9 h-9 rounded-full bg-ctp-blue text-ctp-crust flex items-center justify-center font-bold text-sm mx-auto mb-4">
              {step.number}
            </div>
            <h3 className="text-[15px] font-semibold mb-2">{step.title}</h3>
            <p className="text-[13px] text-ctp-subtext0 leading-relaxed">
              {step.description}
            </p>
            {i < steps.length - 1 && (
              <span className="absolute -right-4 top-1/2 -translate-y-1/2 text-ctp-surface1 text-lg z-10 max-md:hidden">
                &#x2192;
              </span>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}
