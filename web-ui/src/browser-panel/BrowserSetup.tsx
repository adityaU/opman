import React, { useCallback, useState } from "react";
import { Check, Clipboard, ExternalLink, MonitorCog } from "lucide-react";
import type { BrowserInstallGuide } from "../api/client";

interface BrowserSetupProps {
  readonly guide: BrowserInstallGuide;
  readonly onRetry: () => void;
}

export const BrowserSetup: React.FC<BrowserSetupProps> = React.memo(function BrowserSetup({
  guide,
  onRetry,
}) {
  const [copied, setCopied] = useState<string | null>(null);

  const copy = useCallback(async (command: string) => {
    try {
      await navigator.clipboard.writeText(command);
      setCopied(command);
      window.setTimeout(() => setCopied((current) => (current === command ? null : current)), 1600);
    } catch {
      setCopied(null);
    }
  }, []);

  return (
    <div className="bwp-error bwp-setup" role="alert">
      <div className="bwp-setup-mark" aria-hidden="true">
        <MonitorCog size={20} />
      </div>
      <div className="bwp-setup-content">
        <p className="bwp-setup-kicker">Browser setup · {guide.platform}</p>
        <h2>{guide.title}</h2>
        <p className="bwp-setup-summary">{guide.summary}</p>

        {guide.issue.kind !== "no_browser" ? (
          <div className="bwp-setup-override">
            <p className="bwp-setup-override-title">The browser path you set needs attention</p>
            <p className="bwp-setup-override-detail">
              {guide.issue.kind === "override_missing"
                ? "The configured path does not exist."
                : "The configured file is not executable."}
            </p>
            <code className="bwp-setup-override-path">{guide.issue.path}</code>
            <p className="bwp-setup-override-help">
              Update this path or unset <code>{guide.env_var}</code>, then try again.
            </p>
          </div>
        ) : null}

        <ol className="bwp-setup-steps">
          {guide.steps.map((step) => (
            <li key={step}>{step}</li>
          ))}
        </ol>

        {guide.commands.length > 0 ? (
          <div className="bwp-setup-commands">
            {guide.commands.map(({ label, command }) => (
              <div className="bwp-setup-command" key={command}>
                <span className="bwp-setup-command-label">{label}</span>
                <code>{command}</code>
                <button
                  type="button"
                  className="bwp-setup-copy"
                  onClick={() => void copy(command)}
                  aria-label={`Copy ${label} command`}
                  title="Copy command"
                >
                  {copied === command ? <Check size={13} /> : <Clipboard size={13} />}
                </button>
              </div>
            ))}
          </div>
        ) : null}

        <p className="bwp-setup-path">
          Installed somewhere else? Set <code>{guide.env_var}</code> to the browser executable
          before starting opman.
        </p>

        <div className="bwp-setup-actions">
          <button type="button" className="bwp-retry" onClick={onRetry}>
            Try again
          </button>
          <a href={guide.docs_url} target="_blank" rel="noreferrer noopener">
            Browser install docs <ExternalLink size={13} />
          </a>
        </div>
      </div>
    </div>
  );
});
