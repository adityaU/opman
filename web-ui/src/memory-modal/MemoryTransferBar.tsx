import React, { useCallback, useRef, useState } from "react";
import { Download, Upload, X } from "lucide-react";
import { createPersonalMemory, fetchPersonalMemory } from "../api";
import type { MemoryScope, PersonalMemoryItem } from "../api";

interface MemoryExportItem {
  label: string;
  content: string;
  scope: string;
  project_index?: number | null;
  session_id?: string | null;
}

interface MemoryExportEnvelope {
  version: number;
  exported_at: string;
  items: MemoryExportItem[];
}

interface Props {
  items: PersonalMemoryItem[];
  setItems: React.Dispatch<React.SetStateAction<PersonalMemoryItem[]>>;
  activeProjectIndex: number;
  activeSessionId: string | null;
}

type Panel = "download" | "upload" | null;

export function MemoryTransferBar({
  items,
  setItems,
  activeProjectIndex,
  activeSessionId,
}: Props) {
  const [panel, setPanel] = useState<Panel>(null);
  const [dlGlobal, setDlGlobal] = useState(true);
  const [dlProject, setDlProject] = useState(true);
  const [dlSession, setDlSession] = useState(true);
  const [ulStatus, setUlStatus] = useState("");
  const [busy, setBusy] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const handleDownload = useCallback(async () => {
    if (!dlGlobal && !dlProject && !dlSession) return;
    setBusy(true);
    try {
      const resp = await fetchPersonalMemory();
      const all = resp?.memory ?? [];
      const filtered: MemoryExportItem[] = all
        .filter((m) => {
          if (m.scope === "global") return dlGlobal;
          if (m.scope === "project") return dlProject;
          if (m.scope === "session") return dlSession;
          return false;
        })
        .map((m) => ({
          label: m.label,
          content: m.content,
          scope: m.scope,
          project_index: m.project_index,
          session_id: m.session_id,
        }));
      if (filtered.length === 0) return;
      const envelope: MemoryExportEnvelope = {
        version: 1,
        exported_at: new Date().toISOString(),
        items: filtered,
      };
      triggerDownload(JSON.stringify(envelope, null, 2), "opman-memory.json");
    } finally {
      setBusy(false);
    }
  }, [dlGlobal, dlProject, dlSession]);

  const handleFile = useCallback(
    async (ev: React.ChangeEvent<HTMLInputElement>) => {
      const file = ev.target.files?.[0];
      if (!file) return;
      setBusy(true);
      setUlStatus("Reading file...");
      try {
        const text = await file.text();
        const envelope: MemoryExportEnvelope = JSON.parse(text);
        if (!Array.isArray(envelope.items)) {
          setUlStatus("Invalid JSON: missing items array");
          return;
        }
        const total = envelope.items.length;
        let imported = 0;
        for (const item of envelope.items) {
          try {
            const scope = item.scope as MemoryScope;
            const created = await createPersonalMemory({
              label: item.label,
              content: item.content,
              scope,
              project_index:
                scope === "project" || scope === "session"
                  ? (item.project_index ?? activeProjectIndex)
                  : null,
              session_id:
                scope === "session"
                  ? (item.session_id ?? activeSessionId)
                  : null,
            });
            setItems((prev) => [...prev, created]);
            imported++;
          } catch { /* skip failed items */ }
          setUlStatus(`${imported}/${total} imported`);
        }
        setUlStatus(`Done — ${imported} imported`);
      } catch (err) {
        setUlStatus(`Invalid JSON: ${err instanceof Error ? err.message : String(err)}`);
      } finally {
        setBusy(false);
        if (fileRef.current) fileRef.current.value = "";
      }
    },
    [activeProjectIndex, activeSessionId, setItems],
  );

  return (
    <div className="memory-transfer-bar">
      {panel === null && (
        <div className="memory-transfer-triggers">
          <kbd>Up/Down</kbd> Navigate <kbd>Enter</kbd> Edit <kbd>Esc</kbd> Close
          <span className="memory-transfer-spacer" />
          <button
            className="memory-transfer-btn"
            onClick={() => setPanel("download")}
            title="Export session instructions to JSON"
          >
            <Download size={13} /> Export
          </button>
          <button
            className="memory-transfer-btn"
            onClick={() => setPanel("upload")}
            title="Import session instructions from JSON"
          >
            <Upload size={13} /> Import
          </button>
        </div>
      )}

      {panel === "download" && (
        <div className="memory-transfer-panel">
          <div className="memory-transfer-panel-header">
            <span className="memory-transfer-panel-title">Export Memories</span>
            <button className="memory-transfer-close" onClick={() => setPanel(null)}>
              <X size={13} />
            </button>
          </div>
          <div className="memory-transfer-scopes">
            <label className="memory-transfer-scope">
              <input type="checkbox" checked={dlGlobal} onChange={(e) => setDlGlobal(e.target.checked)} /> Global
            </label>
            <label className="memory-transfer-scope">
              <input type="checkbox" checked={dlProject} onChange={(e) => setDlProject(e.target.checked)} /> Project
            </label>
            <label className="memory-transfer-scope">
              <input type="checkbox" checked={dlSession} onChange={(e) => setDlSession(e.target.checked)} /> Session
            </label>
          </div>
          <button
            className="memory-create-btn memory-transfer-action"
            onClick={handleDownload}
            disabled={busy || (!dlGlobal && !dlProject && !dlSession)}
          >
            <Download size={14} />
            {busy ? " Exporting..." : " Download JSON"}
          </button>
        </div>
      )}

      {panel === "upload" && (
        <div className="memory-transfer-panel">
          <div className="memory-transfer-panel-header">
            <span className="memory-transfer-panel-title">Import Memories</span>
            <button
              className="memory-transfer-close"
              onClick={() => { setPanel(null); setUlStatus(""); }}
            >
              <X size={13} />
            </button>
          </div>
          <label className="memory-transfer-file-label">
            <input
              ref={fileRef}
              type="file"
              accept=".json"
              className="memory-transfer-file-input"
              onChange={handleFile}
              disabled={busy}
            />
            <span className="memory-create-btn memory-transfer-action">
              <Upload size={14} />
              {busy ? " Importing..." : " Choose JSON file"}
            </span>
          </label>
          {ulStatus && <span className="memory-transfer-status">{ulStatus}</span>}
        </div>
      )}
    </div>
  );
}

function triggerDownload(content: string, filename: string) {
  const blob = new Blob([content], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.style.display = "none";
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
