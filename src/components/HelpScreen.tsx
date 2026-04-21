interface Props {
  onBack: () => void;
  backLabel?: string;
}

export function HelpScreen({ onBack, backLabel }: Props) {
  return (
    <div className="settings-page">
      <div className="settings-page-content">
        <div className="settings-page-header">
          <button className="btn-text" onClick={onBack}>
            &larr; {backLabel ?? "Back"}
          </button>
          <h1 style={{ marginTop: 8 }}>Help</h1>
        </div>

        <div className="help-section">
          <h2>Quick Start</h2>
          <ol className="help-steps">
            <li>
              <strong>Add a repo</strong> — Go to Manage Hives and paste a
              GitHub URL (HTTPS, SSH, or <code>owner/repo</code>).
            </li>
            <li>
              <strong>Organize with nests</strong> — If you want structure,
              use the <code>+ Nest</code> action in the COMBS header to create
              groups like bugfixes, clients, or
              experiments inside the hive.
            </li>
            <li>
              <strong>Create a workspace</strong> — Select your hive, click
              "+ New Comb", pick a name and branch. Beehive clones it into
              an isolated directory.
            </li>
            <li>
              <strong>Open terminals</strong> — Click a comb to open it.
              Use the header buttons to add terminal or agent panes.
            </li>
          </ol>
        </div>

        <div className="help-section">
          <h2>How It Works</h2>

          <div className="help-concept">
            <h3>Hives</h3>
            <p>
              A hive is a registered GitHub repo. It is the top-level container
              for everything you do in one project: nests, combs, and panes.
            </p>
          </div>

          <div className="help-concept">
            <h3>Nests</h3>
            <p>
              Nests are optional groups inside a hive. Use them to organize
              related combs, like bugfixes, experiments, or client work,
              without changing how the combs behave.
            </p>
          </div>

          <div className="help-concept">
            <h3>Combs</h3>
            <p>
              Each comb is a full git clone on its own branch. Combs can live
              directly in a hive or inside a nest, and they stay completely
              isolated from each other.
            </p>
          </div>

          <div className="help-concept">
            <h3>Panes</h3>
            <p>
              Each comb has a grid of panes. Add terminals for shell access
              or agent panes that launch your configured command (e.g. Claude
              Code). Layouts save automatically.
            </p>
          </div>
        </div>

        <div className="help-section">
          <h2>Good to Know</h2>
          <ul className="help-tips">
            <li>
              Switching between combs or hives keeps all terminals alive
              in the background — nothing is killed.
            </li>
            <li>
              Navigating to Settings, Help, or Manage Hives is the same —
              everything runs underneath the overlay.
            </li>
            <li>
              Combs are real git repos. You can push, pull, rebase,
              switch branches — anything you'd do in a normal clone.
            </li>
            <li>
              Closing a pane (<code>exit</code> or the x button) removes
              it from the grid. The layout updates on disk automatically.
            </li>
            <li>
              You can duplicate a comb with the copy icon in the sidebar.
              This does a full <code>cp&nbsp;-r</code> including uncommitted work.
            </li>
            <li>
              Use <code>+ Nest</code> to create a nest, then right-click a comb
              and use <code>Add to Nest</code> to place it.
            </li>
            <li>
              Deleting a comb removes it from disk permanently. There is
              no undo.
            </li>
          </ul>
        </div>

        <div className="help-section">
          <h2>Requirements</h2>
          <p>
            <code>git</code> and <code>gh</code> (GitHub CLI) must be
            installed and on your PATH. The <code>gh</code> CLI must be
            authenticated — run <code>gh auth login</code> if needed.
            Check status in Settings &rarr; Dependencies.
          </p>
        </div>
      </div>
    </div>
  );
}
