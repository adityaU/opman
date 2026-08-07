import type { CommandDef } from "../types";

/** The code editor and its rich-file renderers. */
export const EDITOR_COMMANDS: readonly CommandDef[] = [
  { id: "editor.save", title: "Save File", category: "Editor", when: "editorDirty", label: "save" },
  { id: "editor.saveAll", title: "Save All Files", category: "Editor", when: "anyDirty", label: "save all" },
  { id: "editor.revert", title: "Revert File", category: "Editor", when: "editorDirty", label: "revert" },
  { id: "editor.close", title: "Close File", category: "Editor", when: "editorOpen", label: "close" },
  { id: "editor.listOpenFiles", title: "Show Open Files", category: "Editor", label: "buffers" },
  { id: "editor.nextFile", title: "Next Open File", category: "Editor", when: "editorOpen" },
  { id: "editor.previousFile", title: "Previous Open File", category: "Editor", when: "editorOpen" },
  { id: "editor.copyContents", title: "Copy File Contents", category: "Editor", when: "editorOpen", label: "file" },
  { id: "editor.sendToChat", title: "Send File to Chat", category: "Editor", when: "editorOpen", label: "send to chat" },
  { id: "editor.togglePreview", title: "Toggle Preview", category: "Editor", when: "editorPreviewable", label: "preview" },
  { id: "editor.toggleComment", title: "Toggle Line Comment", category: "Editor", when: "editorOpen" },
  { id: "editor.undo", title: "Undo", category: "Editor", when: "editorOpen" },
  { id: "editor.redo", title: "Redo", category: "Editor", when: "editorOpen" },
  { id: "editor.find", title: "Find in File", category: "Editor", when: "editorOpen" },
  { id: "editor.replace", title: "Replace in File", category: "Editor", when: "editorOpen" },
];

/** Language-server backed actions. */
export const LSP_COMMANDS: readonly CommandDef[] = [
  { id: "lsp.goToDefinition", title: "Go to Definition", category: "Language", when: "editorOpen", label: "definition" },
  { id: "lsp.findReferences", title: "Find References", category: "Language", when: "editorOpen", label: "references" },
  { id: "lsp.hover", title: "Show Hover Info", category: "Language", when: "editorOpen", label: "hover" },
  { id: "lsp.rename", title: "Rename Symbol", category: "Language", when: "editorOpen", label: "rename" },
  { id: "lsp.format", title: "Format Document", category: "Language", when: "editorOpen", label: "format" },
  { id: "lsp.codeAction", title: "Quick Fix…", category: "Language", when: "editorOpen", label: "action" },
  { id: "lsp.nextDiagnostic", title: "Next Problem", category: "Language", when: "editorOpen" },
  { id: "lsp.previousDiagnostic", title: "Previous Problem", category: "Language", when: "editorOpen" },
  { id: "lsp.diagnosticsList", title: "Show Problems", category: "Language", label: "problems" },
];

/** Document, spreadsheet, viewer and markup renderers. */
export const RICH_FILE_COMMANDS: readonly CommandDef[] = [
  { id: "doc.bold", title: "Bold", category: "Document", when: "focus==document" },
  { id: "doc.italic", title: "Italic", category: "Document", when: "focus==document" },
  { id: "doc.underline", title: "Underline", category: "Document", when: "focus==document" },
  { id: "sheet.addRow", title: "Add Row", category: "Spreadsheet", when: "focus==sheet", label: "add row" },
  { id: "sheet.addColumn", title: "Add Column", category: "Spreadsheet", when: "focus==sheet", label: "add column" },
  { id: "viewer.zoomIn", title: "Zoom In", category: "Viewer", when: "focus==viewer" },
  { id: "viewer.zoomOut", title: "Zoom Out", category: "Viewer", when: "focus==viewer" },
  { id: "viewer.zoomReset", title: "Reset Zoom", category: "Viewer", when: "focus==viewer" },
  { id: "viewer.nextPage", title: "Next Page", category: "Viewer", when: "focus==viewer" },
  { id: "viewer.previousPage", title: "Previous Page", category: "Viewer", when: "focus==viewer" },
  { id: "markup.undo", title: "Undo Markup Stroke", category: "Viewer", when: "markupOpen" },
  { id: "markup.save", title: "Save Markup", category: "Viewer", when: "markupOpen" },
  { id: "markup.cancel", title: "Cancel Markup", category: "Viewer", when: "markupOpen" },
];

/** The file explorer tree and its finder. */
export const EXPLORER_COMMANDS: readonly CommandDef[] = [
  { id: "explorer.moveDown", title: "Move Down", category: "Explorer", when: "focus==explorer" },
  { id: "explorer.moveUp", title: "Move Up", category: "Explorer", when: "focus==explorer" },
  { id: "explorer.expand", title: "Expand Folder", category: "Explorer", when: "focus==explorer" },
  { id: "explorer.collapse", title: "Collapse Folder", category: "Explorer", when: "focus==explorer" },
  { id: "explorer.open", title: "Open File", category: "Explorer", when: "focus==explorer" },
  { id: "explorer.collapseAll", title: "Collapse All Folders", category: "Explorer", when: "focus==explorer" },
  { id: "explorer.newFile", title: "New File", category: "Explorer", label: "new file" },
  { id: "explorer.newFolder", title: "New Folder", category: "Explorer", label: "new folder" },
  { id: "explorer.rename", title: "Rename", category: "Explorer", when: "focus==explorer" },
  { id: "explorer.delete", title: "Delete", category: "Explorer", when: "focus==explorer" },
  { id: "explorer.copyPath", title: "Copy Path", category: "Explorer", when: "focus==explorer", label: "path" },
  { id: "explorer.upload", title: "Upload Files", category: "Explorer", label: "upload" },
  { id: "explorer.download", title: "Download", category: "Explorer", when: "focus==explorer", label: "download" },
  { id: "explorer.reload", title: "Reload", category: "Explorer", label: "reload" },
  { id: "explorer.contextMenu", title: "Show Context Menu", category: "Explorer", when: "focus==explorer" },
  { id: "explorer.clearSearch", title: "Clear Search", category: "Explorer", when: "explorerFinderActive" },
];
