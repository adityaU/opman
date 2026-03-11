import React, { useEffect, useState } from "react";
import { computeMissionHandoff } from "../api";
import type { HandoffBrief } from "../api/intelligence";
import type { MissionHandoffCardProps } from "./types";
import { toPermissionInputs, toQuestionInputs } from "../hooks/intelligenceAdapters";

export function MissionHandoffCard({
  mission,
  permissions,
  questions,
  onOpenInbox,
  onOpenMissionSource,
}: MissionHandoffCardProps) {
  const [brief, setBrief] = useState<HandoffBrief | null>(null);

  useEffect(() => {
    computeMissionHandoff({
      mission_id: mission.id,
      permissions: toPermissionInputs(permissions),
      questions: toQuestionInputs(questions),
    })
      .then(setBrief)
      .catch(() => {});
  }, [mission.id, permissions, questions]);

  if (!brief) return null;

  return (
    <div className="mission-handoff">
      <div className="mission-handoff-title">Resume Brief</div>
      <div className="mission-handoff-summary">{brief.summary}</div>
      {brief.blockers.length > 0 && (
        <div className="mission-handoff-blockers">
          <span className="mission-handoff-label">Blockers</span>
          {brief.blockers.map((blocker) => (
            <span key={blocker} className="mission-handoff-chip mission-handoff-chip-blocker">
              {blocker}
            </span>
          ))}
        </div>
      )}
      <div className="mission-handoff-recent">
        <span className="mission-handoff-label">Recent</span>
        {brief.recent_changes.map((change) => (
          <span key={change} className="mission-handoff-chip">{change}</span>
        ))}
      </div>
      <div className="mission-handoff-footer">
        <span className="mission-handoff-next">Next: {brief.next_action}</span>
        {brief.blockers.length > 0 && onOpenInbox && (
          <button className="mission-handoff-btn" onClick={onOpenInbox}>
            Open inbox
          </button>
        )}
      </div>
      {brief.links.length > 0 && (
        <div className="mission-handoff-links">
          {brief.links.map((link) => (
            <button
              key={`${link.kind}:${link.source_id ?? link.label}`}
              className="mission-handoff-link"
              onClick={() => {
                if (link.kind === "mission" && link.source_id && onOpenMissionSource) {
                  onOpenMissionSource(link.source_id);
                  return;
                }
                if ((link.kind === "permission" || link.kind === "question") && onOpenInbox) {
                  onOpenInbox();
                }
              }}
            >
              {link.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
