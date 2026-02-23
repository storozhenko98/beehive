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
          <h2>What is Beehive?</h2>
          <p>
            Beehive is a desktop app for managing multiple coding workspaces
            across your repositories. It lets you work on several branches or
            features in parallel, each with its own isolated set of terminals
            and AI agent panes.
          </p>
        </div>

        <div className="help-section">
          <h2>Core Concepts</h2>

          <div className="help-concept">
            <h3>Hives</h3>
            <p>
              A <strong>hive</strong> represents a GitHub repository. When you
              add a hive, Beehive registers the repo so you can create
              workspaces from it. You can manage multiple hives — one per repo
              you work on.
            </p>
            <div className="help-flow">Manage Hives &rarr; + Add Hive &rarr; paste a repo URL</div>
          </div>

          <div className="help-concept">
            <h3>Combs</h3>
            <p>
              A <strong>comb</strong> is an isolated workspace clone of a hive.
              Each comb lives in its own directory on disk, checked out to a
              specific branch. This means you can have multiple combs for the
              same repo — one per feature branch, bugfix, or experiment —
              without them interfering with each other.
            </p>
            <div className="help-flow">Select a hive &rarr; + New Comb &rarr; name it &amp; pick a branch</div>
          </div>

          <div className="help-concept">
            <h3>Panes</h3>
            <p>
              Each comb opens with a grid of <strong>panes</strong>. A pane is
              either a <strong>terminal</strong> (a regular shell) or an{" "}
              <strong>agent</strong> (launches Claude Code). You can add and
              remove panes freely. Your pane layout is saved automatically and
              restored when you reopen the comb.
            </p>
            <div className="help-flow">+ Terminal / + Agent buttons in the workspace header</div>
          </div>
        </div>

        <div className="help-section">
          <h2>Workflow</h2>
          <ol className="help-steps">
            <li>
              <strong>Add a hive</strong> — Go to Manage Hives and add a
              repository by its URL (e.g. <code>owner/repo</code>,
              GitHub HTTPS, or SSH URL).
            </li>
            <li>
              <strong>Create a comb</strong> — Select the hive in the sidebar,
              then click "+ New Comb". Give it a name and choose a branch.
              Beehive clones the repo into an isolated directory.
            </li>
            <li>
              <strong>Work</strong> — Click a comb in the sidebar to open it.
              Use terminal panes for shell commands and agent panes to launch
              Claude Code. Add as many panes as you need.
            </li>
            <li>
              <strong>Switch freely</strong> — Click between combs and hives in
              the sidebar. All your terminals stay alive in the background —
              nothing is lost when you switch.
            </li>
          </ol>
        </div>

        <div className="help-section">
          <h2>Requirements</h2>
          <p>
            Beehive requires <code>git</code> and <code>gh</code> (GitHub CLI)
            to be installed and available on your PATH. The <code>gh</code> CLI
            must be authenticated (<code>gh auth login</code>). You can check
            the status of these in Settings &rarr; Dependencies.
          </p>
        </div>

        <div className="help-section">
          <h2>Tips</h2>
          <ul className="help-tips">
            <li>
              Combs are full git clones — you can run any git command inside
              them, push, pull, rebase, etc.
            </li>
            <li>
              Closing a pane (x button or typing <code>exit</code>) removes it
              from the grid. The layout saves automatically.
            </li>
            <li>
              Navigating to Manage Hives or Settings doesn't kill your
              terminals — they keep running underneath.
            </li>
            <li>
              Deleting a comb removes it from the sidebar and deletes the
              directory on disk. This cannot be undone.
            </li>
          </ul>
        </div>
      </div>
    </div>
  );
}
