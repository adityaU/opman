import React from "react";
import { Bot, UserRound } from "lucide-react";
import ReactMarkdown from "react-markdown";
import { markdownComponents, REMARK_PLUGINS } from "../message-turn/CodeBlock";
import type { Note } from "../api/kanban";

function fmtTime(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
}

/** A single entry in a task's activity timeline (agent or user note / lane move). */
export const NoteRow: React.FC<{ note: Note; laneName: (id: string | null) => string }> =
  function NoteRow({ note, laneName }) {
    const isUser = note.author === "user";
    const moved = note.lane_from && note.lane_to && note.lane_from !== note.lane_to;
    return (
      <li className={`kanban-note kanban-note-${note.author}`}>
        <span className="kanban-note-icon">
          {isUser ? <UserRound size={13} /> : <Bot size={13} />}
        </span>
        <div className="kanban-note-main">
          <div className="kanban-note-head">
            <span className="kanban-note-author">{isUser ? "You" : "Agent"}</span>
            {moved && (
              <span className="kanban-note-move">
                {laneName(note.lane_from)} → {laneName(note.lane_to)}
              </span>
            )}
            <span className="kanban-note-time">{fmtTime(note.created_at)}</span>
          </div>
          <div className="kanban-note-body">
            <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
              {note.body}
            </ReactMarkdown>
          </div>
        </div>
      </li>
    );
  };
