import React from "react";
import { ArrowRight, Bot, UserRound } from "lucide-react";
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
    const moved = note.lane_from && note.lane_to && note.lane_from !== note.lane_to;

    // Lane-move / pipeline-transition entries read as a slim centered timeline
    // divider, visually distinct from the author message cards below.
    if (moved) {
      return (
        <li className="kanban-note-move-row" title={fmtTime(note.created_at)}>
          <span className="kanban-note-move-pill">
            <span className="kanban-note-move-lanes">
              {laneName(note.lane_from)}
              <ArrowRight size={11} aria-hidden />
              {laneName(note.lane_to)}
            </span>
            {note.body && <span className="kanban-note-move-label">{note.body}</span>}
          </span>
        </li>
      );
    }

    const isUser = note.author === "user";
    return (
      <li className={`kanban-note kanban-note-${note.author}`}>
        <span className="kanban-note-icon">
          {isUser ? <UserRound size={13} /> : <Bot size={13} />}
        </span>
        <div className="kanban-note-main">
          <div className="kanban-note-head">
            <span className="kanban-note-author">{isUser ? "You" : "Agent"}</span>
            <span className="kanban-note-time">{fmtTime(note.created_at)}</span>
          </div>
          <div className="kanban-note-body kanban-note-markdown">
            <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
              {note.body}
            </ReactMarkdown>
          </div>
        </div>
      </li>
    );
  };
