import type { MessageModel } from "../state/messages";
import type { MessageItem } from "../state/types";

export interface MessagesProps {
  readonly state: MessageModel;
}

type MessageTone = "error" | "warning" | "info";

function cellsToText(cells: readonly { readonly text: string }[]): string {
  return cells.map((cell) => cell.text).join("");
}

function messageTone(kind: MessageItem["kind"]): MessageTone {
  switch (kind) {
    case "emsg":
    case "echoerr":
    case "shell_err":
      return "error";
    case "wmsg":
    case "warning":
    case "question":
    case "confirm":
      return "warning";
    default:
      return "info";
  }
}

function toneLabel(tone: MessageTone): string {
  return tone === "error" ? "Error" : tone === "warning" ? "Warning" : "Info";
}

export function Messages({ state }: MessagesProps): React.ReactElement | null {
  const items = state.items.length > 0 ? state.items : state.history;
  if (items.length === 0) return null;
  const history = state.history.length > 0 && state.items.length === state.history.length;
  return (
    <section className="nvim-messages-overlay" aria-label={history ? "Neovim message history" : "Neovim messages"} aria-live="polite">
      <div className="nvim-message-list" data-history={history ? "true" : undefined}>
        {items.map((item, index) => {
          const tone = messageTone(item.kind);
          return (
            <div
              className={`nvim-message nvim-message-${tone}`}
              key={`${item.kind}-${index}`}
              role={tone === "error" ? "alert" : tone === "warning" ? "status" : undefined}
              aria-label={`${toneLabel(tone)} message`}
            >
              <span className="nvim-message-kind" aria-hidden="true">{toneLabel(tone)}</span>
              <span className="nvim-message-content">{cellsToText(item.content)}</span>
            </div>
          );
        })}
      </div>
      {state.scrolled && <span className="nvim-message-scroll" aria-label="Messages scrolled">{state.separator || "More messages"}</span>}
    </section>
  );
}

export default Messages;
