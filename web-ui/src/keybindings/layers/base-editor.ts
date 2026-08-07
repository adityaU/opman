import type { BindingSpec } from "../types";

/** Editor, language server, explorer and the rich-file renderers. */

export const BASE_EDITOR: readonly BindingSpec[] = [
  { key: "mod+s", command: "editor.save", when: "editorDirty" },
  { key: "mod+k s", command: "editor.saveAll", when: "anyDirty" },
  { key: "mod+k u", command: "editor.revert", when: "editorDirty" },
  { key: "mod+w", command: "editor.close", when: "editorOpen" },
  { key: "mod+k mod+b", command: "editor.listOpenFiles" },
  { key: "alt+]", command: "editor.nextFile", when: "editorOpen" },
  { key: "alt+[", command: "editor.previousFile", when: "editorOpen" },
  { key: "mod+k enter", command: "editor.sendToChat", when: "editorOpen" },
  { key: "mod+shift+v", command: "editor.togglePreview", when: "editorPreviewable" },
  { key: "mod+/", command: "editor.toggleComment", when: "focus==editor" },
  { key: "mod+z", command: "editor.undo", when: "focus==editor" },
  { key: "mod+shift+z", command: "editor.redo", when: "focus==editor" },
  { key: "mod+f", command: "editor.find", when: "focus==editor" },
  { key: "mod+h", command: "editor.replace", when: "focus==editor" },
];

export const BASE_LSP: readonly BindingSpec[] = [
  { key: "f12", command: "lsp.goToDefinition", when: "editorOpen" },
  { key: "shift+f12", command: "lsp.findReferences", when: "editorOpen" },
  { key: "mod+k mod+i", command: "lsp.hover", when: "editorOpen" },
  { key: "f2", command: "lsp.rename", when: "focus==editor" },
  { key: "shift+alt+f", command: "lsp.format", when: "editorOpen" },
  { key: "mod+.", command: "lsp.codeAction", when: "editorOpen" },
  { key: "f8", command: "lsp.nextDiagnostic", when: "editorOpen" },
  { key: "shift+f8", command: "lsp.previousDiagnostic", when: "editorOpen" },
  { key: "mod+shift+m", command: "lsp.diagnosticsList" },
];

export const BASE_RICH_FILE: readonly BindingSpec[] = [
  { key: "mod+b", command: "doc.bold", when: "focus==document" },
  { key: "mod+i", command: "doc.italic", when: "focus==document" },
  { key: "mod+u", command: "doc.underline", when: "focus==document" },
  { key: "mod+enter", command: "sheet.addRow", when: "focus==sheet" },
  { key: "mod+shift+enter", command: "sheet.addColumn", when: "focus==sheet" },
  { key: "mod+=", command: "viewer.zoomIn", when: "focus==viewer" },
  { key: "mod+-", command: "viewer.zoomOut", when: "focus==viewer" },
  { key: "mod+0", command: "viewer.zoomReset", when: "focus==viewer" },
  { key: "pagedown", command: "viewer.nextPage", when: "focus==viewer" },
  { key: "pageup", command: "viewer.previousPage", when: "focus==viewer" },
  { key: "mod+z", command: "markup.undo", when: "markupOpen" },
  { key: "mod+s", command: "markup.save", when: "markupOpen" },
  { key: "escape", command: "markup.cancel", when: "markupOpen" },
];

export const BASE_EXPLORER: readonly BindingSpec[] = [
  { key: "down", command: "explorer.moveDown", when: "focus==explorer" },
  { key: "up", command: "explorer.moveUp", when: "focus==explorer" },
  { key: "right", command: "explorer.expand", when: "focus==explorer" },
  { key: "left", command: "explorer.collapse", when: "focus==explorer" },
  { key: "enter", command: "explorer.open", when: "focus==explorer" },
  { key: "mod+k mod+0", command: "explorer.collapseAll", when: "focus==explorer" },
  { key: "mod+alt+n", command: "explorer.newFile", when: "focus==explorer" },
  { key: "mod+alt+shift+n", command: "explorer.newFolder", when: "focus==explorer" },
  { key: "f2", command: "explorer.rename", when: "focus==explorer" },
  { key: "delete", command: "explorer.delete", when: "focus==explorer" },
  { key: "f5", command: "explorer.reload", when: "focus==explorer" },
  { key: "mod+.", command: "explorer.contextMenu", when: "focus==explorer" },
  { key: "escape", command: "explorer.clearSearch", when: "explorerFinderActive" },
];
