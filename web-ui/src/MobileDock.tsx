import React, { Suspense, lazy } from "react";
import { MessageCircle, GitBranch, FileCode, Terminal, Sparkles, PenSquare, Command, Menu } from "lucide-react";
const TerminalPanel = lazy(() => import("./TerminalPanel").then(m => ({ default: m.TerminalPanel })));
const CodeEditorPanel = lazy(() => import("./code-editor"));
const GitPanel = lazy(() => import("./git-panel"));

interface MobileDockProps {
  activePanel: "opencode" | "git" | "editor" | "terminal" | null;
  panelsMounted: Set<string>;
  togglePanel: (panel: "opencode" | "git" | "editor" | "terminal") => void;
  inputHidden: boolean;
  handleComposeButtonTap: () => void;
  dockCollapsed: boolean;
  expandDock: () => void;
  assistantCenterOpen: boolean;
  onOpenAssistantCenter: () => void;
  onOpenCommandPalette: () => void;
  activeSessionId: string | null;
  activeProject: any;
  mcpEditorOpenPath: string | null;
  mcpEditorOpenLine: number | null;
  mcpAgentActivity: Map<string, any>;
  onError: (msg: string) => void;
  onSendToAI: (text: string, images?: any[]) => Promise<void>;
}

export function MobileDock(props: MobileDockProps): React.ReactElement {
  const {
    activePanel, panelsMounted, togglePanel, inputHidden, handleComposeButtonTap,
    dockCollapsed, expandDock,
    assistantCenterOpen, onOpenAssistantCenter, onOpenCommandPalette,
    activeSessionId, activeProject, mcpEditorOpenPath, mcpEditorOpenLine,
    mcpAgentActivity, onError, onSendToAI,
  } = props;

  // Compose button visibility:
  //   "visible"  = input hidden + dock collapsed  (both FABs float independently)
  //   "consumed" = input hidden + dock expanded   (compose animates into the dock)
  //   hidden     = input visible                   (no compose button needed)
  const composeClass = inputHidden
    ? dockCollapsed ? "visible" : "consumed"
    : "";

  return (
    <>
      {/* Compose button — always rendered on mobile; visibility driven by CSS classes */}
      <button
        className={`mobile-compose-btn ${composeClass}`}
        onClick={handleComposeButtonTap}
        aria-label="Compose message"
      >
        <PenSquare size={20} />
      </button>

      {/* Collapsed FAB — round button pinned to bottom-right; hidden when input is visible */}
      <button
        className={`mobile-dock-fab ${dockCollapsed && inputHidden ? "visible" : ""}`}
        onClick={expandDock}
        aria-label="Open navigation"
      >
        <Menu size={22} />
      </button>

      {/* Expanded dock — floating pill with ambient glow */}
      <nav className={`mobile-dock ${dockCollapsed ? "dock-collapsed" : ""}`} aria-label="Navigation">
        <div className="mobile-dock-inner">
          {/* Compose button inside dock — slides in when input is hidden */}
          <button
            className={`mobile-dock-btn dock-compose-btn ${inputHidden && !dockCollapsed ? "dock-compose-visible" : ""}`}
            onClick={handleComposeButtonTap}
            aria-label="Compose message"
          >
            <PenSquare size={20} />
          </button>
          <button className={`mobile-dock-btn ${activePanel === null || activePanel === "opencode" ? "active" : ""}`} onClick={() => togglePanel("opencode")} aria-label="Chat">
            <MessageCircle size={18} />
            <span className="dock-label">Chat</span>
          </button>
          <button className={`mobile-dock-btn ${activePanel === "git" ? "active" : ""}`} onClick={() => togglePanel("git")} aria-label="Git">
            <GitBranch size={18} />
            <span className="dock-label">Git</span>
          </button>
          <button className={`mobile-dock-btn ${activePanel === "editor" ? "active" : ""}`} onClick={() => togglePanel("editor")} aria-label="Editor">
            <FileCode size={18} />
            <span className="dock-label">Editor</span>
          </button>
          <button className={`mobile-dock-btn ${activePanel === "terminal" ? "active" : ""}`} onClick={() => togglePanel("terminal")} aria-label="Terminal">
            <Terminal size={18} />
            <span className="dock-label">Term</span>
          </button>
          <button className={`mobile-dock-btn ${assistantCenterOpen ? "active" : ""}`} onClick={onOpenAssistantCenter} aria-label="Assistant">
            <Sparkles size={18} />
            <span className="dock-label">AI</span>
          </button>
        </div>
      </nav>

      {/* Mobile panel sheets — slide-up modal sheets with glass chrome */}
      {panelsMounted.has("git") && (
        <div className={`mobile-panel-sheet ${activePanel === "git" ? "mobile-panel-active" : ""}`}>
          <div className="mobile-sheet-handle" />
          <div className="mobile-panel-header">
            <GitBranch size={15} />
            <span className="mobile-panel-title">Git</span>
            <button className="mobile-cmd-btn" onClick={onOpenCommandPalette} aria-label="Open command palette">
              <Command size={14} />
            </button>
          </div>
          <Suspense fallback={null}>
            <GitPanel focused={activePanel === "git"} projectPath={activeProject?.path} onError={onError} onSendToAI={onSendToAI} />
          </Suspense>
        </div>
      )}
      {panelsMounted.has("editor") && (
        <div className={`mobile-panel-sheet ${activePanel === "editor" ? "mobile-panel-active" : ""}`}>
          <div className="mobile-sheet-handle" />
          <div className="mobile-panel-header">
            <FileCode size={15} />
            <span className="mobile-panel-title">Editor</span>
            <button className="mobile-cmd-btn" onClick={onOpenCommandPalette} aria-label="Open command palette">
              <Command size={14} />
            </button>
          </div>
          <Suspense fallback={null}>
            <CodeEditorPanel focused={activePanel === "editor"} openFilePath={mcpEditorOpenPath} openLine={mcpEditorOpenLine} projectPath={activeProject?.path} sessionId={activeSessionId} onError={onError} />
          </Suspense>
        </div>
      )}
      {panelsMounted.has("terminal") && (
        <div className={`mobile-panel-sheet ${activePanel === "terminal" ? "mobile-panel-active" : ""}`}>
          <div className="mobile-sheet-handle" />
          <div className="mobile-panel-header">
            <Terminal size={15} />
            <span className="mobile-panel-title">Terminal</span>
            <button className="mobile-cmd-btn" onClick={onOpenCommandPalette} aria-label="Open command palette">
              <Command size={14} />
            </button>
          </div>
          <Suspense fallback={null}>
            <TerminalPanel
              sessionId={activeSessionId}
              onClose={() => togglePanel("terminal")}
              mcpAgentActive={Array.from(mcpAgentActivity.keys()).some(t => t.startsWith("web_terminal"))}
            />
          </Suspense>
        </div>
      )}
    </>
  );
}
