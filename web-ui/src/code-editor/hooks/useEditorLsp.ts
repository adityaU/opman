/**
 * useEditorLsp — binds the LSP calls to the CodeMirror instance.
 *
 * CodeMirror extensions are built once and live for the editor's lifetime,
 * while the open file, session, and diagnostics change constantly. A ref is
 * the join: the extensions read it at call time, so they never capture a stale
 * path, and the extension array stays referentially stable — rebuilding it on
 * every keystroke would tear down and remount the editor.
 */
import { useEffect, useMemo, useRef } from "react";
import { createLspExtensions, pushDiagnostics, type LspBridge } from "../lsp/editorLsp";
import type { EditorLspDiagnostic } from "../types";

interface Args {
  enabled: boolean;
  activeFilePath: string | null;
  activeDiagnostics: EditorLspDiagnostic[];
  editorViewRef: React.MutableRefObject<any>;
  hoverAt: LspBridge["hoverAt"];
  definitionAt: (line: number, col: number) => Promise<boolean>;
  format: () => Promise<void>;
  completeAt: LspBridge["completeAt"];
  triggerCharacters: () => string[];
  referencesAt: LspBridge["referencesAt"];
  renameAt: LspBridge["renameAt"];
  jumpTo: LspBridge["jumpTo"];
  gotoAt: LspBridge["gotoAt"];
  reveal: LspBridge["reveal"];
}

export function useEditorLsp({
  enabled, activeFilePath, activeDiagnostics, editorViewRef,
  hoverAt, definitionAt, format, completeAt, triggerCharacters,
  referencesAt, renameAt, jumpTo, gotoAt, reveal,
}: Args) {
  const bridge = useRef<LspBridge>({
    enabled, diagnostics: () => activeDiagnostics, hoverAt, definitionAt, format, completeAt, triggerCharacters,
    referencesAt, renameAt, jumpTo, gotoAt, reveal,
  });

  // Refresh the handles in place rather than rebuilding the extensions.
  bridge.current.enabled = enabled;
  bridge.current.diagnostics = () => activeDiagnostics;
  bridge.current.hoverAt = hoverAt;
  bridge.current.definitionAt = definitionAt;
  bridge.current.format = format;
  bridge.current.completeAt = completeAt;
  bridge.current.triggerCharacters = triggerCharacters;
  bridge.current.referencesAt = referencesAt;
  bridge.current.renameAt = renameAt;
  bridge.current.jumpTo = jumpTo;
  bridge.current.gotoAt = gotoAt;
  bridge.current.reveal = reveal;

  const extensions = useMemo(() => createLspExtensions(bridge), []);

  useEffect(() => {
    pushDiagnostics(editorViewRef.current, enabled ? activeDiagnostics : []);
  }, [activeDiagnostics, activeFilePath, enabled, editorViewRef]);

  return { extensions };
}
