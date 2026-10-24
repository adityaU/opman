import React, { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { McpServer, McpServerDraft } from "../../api/mcp";
import { CodeModeSwitch } from "./CodeModeSwitch";
import { ToolCatalog } from "./ToolCatalog";

export function CodeModeSettings({ servers, busy, save }: {
  readonly servers: readonly McpServer[];
  readonly busy?: string;
  readonly save: (name: string, draft: McpServerDraft) => Promise<boolean>;
}) {
  const [expanded, setExpanded] = useState<string>();
  const code = servers.find(server => server.name === "code");
  const entries = servers.filter(server => server.name !== "code");

  return <section className="stg-card" aria-labelledby="cm-settings-heading">
    <div className="stg-card-head">
      <div>
        <h3 className="stg-card-title" id="cm-settings-heading">Code mode</h3>
        <p className="stg-card-note">
          Choose which servers and tools JavaScript can call. These settings are independent
          of direct MCP access below. Saved changes apply to the next code mode call.
        </p>
        {code && !code.enabled && <p className="stg-hint">
          Code mode is currently off. Enable the code server below to use these tools.
        </p>}
      </div>
    </div>
    <ul className="stg-rows">
      {entries.map(server => {
        const open = expanded === server.name;
        const disabled = server.codeModeDisabledTools ?? [];
        const panelId = `cm-settings-${encodeURIComponent(server.name)}`;
        return <li key={server.name} className="cm-settings-row">
          <div className="cm-settings-row-head">
            <button type="button" className="cm-settings-disclosure" aria-expanded={open}
              aria-label={`${open ? "Hide" : "Show"} code mode tools for ${server.name}`}
              aria-controls={panelId} onClick={() => setExpanded(open ? undefined : server.name)}>
              {open ? <ChevronDown size={14} aria-hidden="true" /> : <ChevronRight size={14} aria-hidden="true" />}
              <span>{server.name === "opman-management" ? "Opman management" : server.name}</span>
              <span className="stg-hint">{disabled.length ? `${disabled.length} ${disabled.length === 1 ? "tool" : "tools"} off` : "All tools"}</span>
            </button>
            <CodeModeSwitch checked={server.codeMode !== false} busy={busy !== undefined}
              label={`Use ${server.name} in code mode`}
              onChange={enabled => { void save(server.name, { codeMode: enabled }); }} />
          </div>
          <div id={panelId} hidden={!open}>
            {open && <ToolCatalog name={server.name} open controls={{
              disabledTools: disabled,
              busy: busy !== undefined,
              onToggle: (tool, enabled) => {
                const next = disabled.filter(name => name !== tool);
                if (!enabled) next.push(tool);
                void save(server.name, { codeModeDisabledTools: next });
              },
            }} />}
          </div>
        </li>;
      })}
    </ul>
  </section>;
}
