export type CodePreviewKind = "markdown" | "html" | "svg" | "csv" | "mermaid";

export function codePreviewKind(language: string): CodePreviewKind | null {
  switch (language.toLowerCase()) {
    case "md": case "markdown": return "markdown";
    case "html": case "htm": return "html";
    case "svg": return "svg";
    case "csv": return "csv";
    case "mermaid": case "mmd": return "mermaid";
    default: return null;
  }
}
