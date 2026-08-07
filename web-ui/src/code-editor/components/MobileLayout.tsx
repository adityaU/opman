/**
 * MobileLayout — the phone-sized file browser, and the editor once a file is
 * open.
 *
 * The browser keeps the directory-at-a-time model, which is right for a narrow
 * column, and adds the one thing that model cannot give you: a search that
 * reaches the whole project, so finding a file no longer means guessing which
 * folder it is in and tapping down to it. Row actions stay on a swipe, and
 * everything that used to be a cramped inline strip — rename, create, delete,
 * the actions menu — is now a bottom sheet within thumb reach.
 */
import { useRef, useState } from "react";
import { Loader2, FileQuestion } from "lucide-react";
import type {
  FileReadResponse, FileRenderType, EditorLspDiagnostic,
  EditorViewMode, BreadcrumbEntry, FileEntry, OpenFileEntry,
} from "../types";
import { EditorToolbar } from "./EditorToolbar";
import { EditorBody } from "./EditorBody";
import { MobileBrowserHeader } from "../explorer/mobile/MobileBrowserHeader";
import { MobileFileRow } from "../explorer/mobile/MobileFileRow";
import {
  ActionsSheet, DeleteSheet, NameSheet, buildFolderActions,
} from "../explorer/mobile/MobileSheets";
import { useExplorerFinder } from "../explorer/useExplorerFinder";

interface Props {
  editorRef: React.RefObject<HTMLDivElement>;
  breadcrumbs: BreadcrumbEntry[];
  entries: FileEntry[];
  loadingDir: boolean;
  loadDirectory: (path: string) => Promise<void>;
  loadFile: (path: string, line?: number | null) => Promise<void>;
  currentPath: string;
  openFile: FileReadResponse | null;
  fileRenderType: FileRenderType;
  isModified: boolean;
  currentContent: string;
  activeView: EditorViewMode;
  setActiveView: (mode: EditorViewMode) => void;
  extensions: any[];
  onEditorChange: (value: string) => void;
  onCreateEditor: (view: any) => void;
  onUpdate: (update: any) => void;
  loadingFile: boolean;
  languageLoading: boolean;
  lspAvailable: boolean;
  lspBusy: null | "hover" | "definition" | "format";
  activeDiagnostics: EditorLspDiagnostic[];
  hoverText: string | null;
  handleHover: () => void;
  handleDefinition: () => void;
  handleFormatWithLsp: () => void;
  onJumpToLine?: (line: number, col: number) => void;
  saveStatus: "saved" | "modified" | null;
  saving: boolean;
  handleSave: () => void;
  handleRevert: () => void;
  onEntryClick: (entry: FileEntry) => void;
  onBackToBrowser: () => void;
  onCreateFile?: (parentDir: string, name: string) => void;
  onCreateDir?: (parentDir: string, name: string) => void;
  onDeleteFile?: (path: string) => void;
  onDeleteDir?: (path: string) => void;
  onUploadFiles?: (dir: string, files: FileList | File[]) => void;
  onReloadRoot?: () => void;
  onRename?: (oldPath: string, newName: string, isDir: boolean) => void;
  onDownloadFile?: (path: string) => void;
  onDownloadDir?: (path: string) => void;
  fileActionBusy?: boolean;
  activeEntry?: OpenFileEntry | null;
  setOpenFiles?: React.Dispatch<React.SetStateAction<OpenFileEntry[]>>;
}

type Sheet =
  | { kind: "actions" }
  | { kind: "create"; type: "file" | "dir" }
  | { kind: "rename"; path: string; name: string; isDir: boolean }
  | { kind: "delete"; path: string; name: string; isDir: boolean };

export function MobileLayout(p: Props) {
  const uploadRef = useRef<HTMLInputElement>(null);
  const [sheet, setSheet] = useState<Sheet | null>(null);
  const finder = useExplorerFinder();

  // ── Editor ────────────────────────────────────────────
  if (p.openFile) {
    return (
      <div className="code-editor-panel" ref={p.editorRef}>
        <EditorToolbar
          openFile={p.openFile} fileRenderType={p.fileRenderType}
          isModified={p.isModified} isDesktop={false}
          activeView={p.activeView} setActiveView={p.setActiveView}
          lspAvailable={p.lspAvailable} lspBusy={p.lspBusy}
          activeDiagnostics={p.activeDiagnostics}
          handleHover={p.handleHover} handleDefinition={p.handleDefinition}
          handleFormatWithLsp={p.handleFormatWithLsp}
          saveStatus={p.saveStatus} saving={p.saving}
          handleSave={p.handleSave} handleRevert={p.handleRevert}
          onBackToBrowser={p.onBackToBrowser}
        />
        <EditorBody
          openFile={p.openFile} fileRenderType={p.fileRenderType}
          currentContent={p.currentContent} activeView={p.activeView}
          extensions={p.extensions} onEditorChange={p.onEditorChange}
          onCreateEditor={p.onCreateEditor} onUpdate={p.onUpdate}
          loadingFile={p.loadingFile} languageLoading={p.languageLoading}
          activeDiagnostics={p.activeDiagnostics} hoverText={p.hoverText}
          lspAvailable={p.lspAvailable} onJumpToLine={p.onJumpToLine}
          activeEntry={p.activeEntry} setOpenFiles={p.setOpenFiles}
        />
      </div>
    );
  }

  // ── Browser ───────────────────────────────────────────
  const folderActions = buildFolderActions({
    onNewFile: p.onCreateFile ? () => setSheet({ kind: "create", type: "file" }) : undefined,
    onNewFolder: p.onCreateDir ? () => setSheet({ kind: "create", type: "dir" }) : undefined,
    onUpload: p.onUploadFiles ? () => uploadRef.current?.click() : undefined,
    onReload: p.onReloadRoot,
  });

  const folderName = p.breadcrumbs[p.breadcrumbs.length - 1]?.label ?? "Files";

  return (
    <div className="code-editor-panel xplm">
      <MobileBrowserHeader
        breadcrumbs={p.breadcrumbs}
        query={finder.query}
        searching={finder.searching}
        hasActions={folderActions.length > 0}
        onQueryChange={finder.setQuery}
        onQueryClear={finder.clear}
        onNavigate={(path) => { finder.clear(); p.loadDirectory(path); }}
        onOpenActions={() => setSheet({ kind: "actions" })}
      />

      <input
        ref={uploadRef}
        type="file"
        multiple
        style={{ display: "none" }}
        onChange={(event) => {
          if (!event.target.files?.length) return;
          p.onUploadFiles?.(p.currentPath, event.target.files);
          event.target.value = "";
        }}
      />

      <div className="xplm-list">
        {finder.active ? renderResults() : renderDirectory()}
      </div>

      {sheet?.kind === "actions" && (
        <ActionsSheet folder={folderName} actions={folderActions} onClose={() => setSheet(null)} />
      )}
      {sheet?.kind === "create" && (
        <NameSheet
          title={sheet.type === "file" ? "New file" : "New folder"}
          initial=""
          placeholder={sheet.type === "file" ? "filename.ext" : "folder name"}
          confirmLabel="Create"
          onSubmit={(name) => {
            if (sheet.type === "file") p.onCreateFile?.(p.currentPath, name);
            else p.onCreateDir?.(p.currentPath, name);
          }}
          onClose={() => setSheet(null)}
        />
      )}
      {sheet?.kind === "rename" && (
        <NameSheet
          title="Rename"
          initial={sheet.name}
          selectStem={!sheet.isDir}
          confirmLabel="Rename"
          onSubmit={(name) => p.onRename?.(sheet.path, name, sheet.isDir)}
          onClose={() => setSheet(null)}
        />
      )}
      {sheet?.kind === "delete" && (
        <DeleteSheet
          name={sheet.name}
          isDir={sheet.isDir}
          onConfirm={() => {
            if (sheet.isDir) p.onDeleteDir?.(sheet.path);
            else p.onDeleteFile?.(sheet.path);
          }}
          onClose={() => setSheet(null)}
        />
      )}
    </div>
  );

  function rowActions(entry: FileEntry) {
    return {
      onRename: p.onRename
        ? () => setSheet({ kind: "rename", path: entry.path, name: entry.name, isDir: entry.is_dir })
        : undefined,
      onDownload: entry.is_dir
        ? (p.onDownloadDir ? () => p.onDownloadDir!(entry.path) : undefined)
        : (p.onDownloadFile ? () => p.onDownloadFile!(entry.path) : undefined),
      onDelete: (entry.is_dir ? p.onDeleteDir : p.onDeleteFile)
        ? () => setSheet({ kind: "delete", path: entry.path, name: entry.name, isDir: entry.is_dir })
        : undefined,
    };
  }

  function renderDirectory() {
    if (p.loadingDir) {
      return (
        <div className="xplm-state">
          <Loader2 size={18} className="spin" />
          <span>Loading files…</span>
        </div>
      );
    }
    if (p.entries.length === 0) {
      return (
        <div className="xplm-state">
          <span>This folder is empty.</span>
          <span className="xplm-state-hint">Use the menu above to add a file or folder.</span>
        </div>
      );
    }
    return p.entries.map((entry) => (
      <MobileFileRow
        key={entry.path}
        entry={entry}
        onOpen={p.onEntryClick}
        {...rowActions(entry)}
      />
    ));
  }

  function renderResults() {
    const query = finder.query.trim();
    if (finder.error) {
      return <div className="xplm-state xplm-state-error">{finder.error}</div>;
    }
    if (finder.results.length === 0) {
      if (finder.searching) return <div className="xplm-state"><Loader2 size={18} className="spin" /><span>Searching…</span></div>;
      return (
        <div className="xplm-state">
          <FileQuestion size={22} strokeWidth={1.4} />
          <span>No file matches <strong>{query}</strong>.</span>
          <span className="xplm-state-hint">Try part of the name, or a folder it sits in.</span>
        </div>
      );
    }
    return (
      <>
        <div className="xplm-results-count">
          {finder.results.length} {finder.results.length === 1 ? "match" : "matches"}
        </div>
        {finder.results.map((result) => {
          const entry: FileEntry = {
            name: result.name,
            path: result.path,
            is_dir: result.is_dir,
            size: 0,
          } as FileEntry;
          const parent = result.path.slice(0, Math.max(0, result.path.length - result.name.length - 1));
          return (
            <MobileFileRow
              key={result.path}
              entry={entry}
              query={query}
              subtitle={parent || undefined}
              onOpen={() => {
                finder.clear();
                if (result.is_dir) p.loadDirectory(result.path);
                else p.loadFile(result.path);
              }}
            />
          );
        })}
      </>
    );
  }
}
