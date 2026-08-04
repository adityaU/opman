import React, { useState } from "react";
import type { MessageGroup } from "./types";

type Props = { groups: MessageGroup[]; onSelect: (index: number) => void };

function groupWeight(group: MessageGroup): number {
  return group.messages.reduce((total, message) => total + message.parts.reduce((size, part) => size + (part.text?.length || 0), 0), 0);
}
function groupPreview(group: MessageGroup): string {
  return group.messages.flatMap((message) => message.parts).map((part) => part.text || "").join(" ").replace(/\s+/g, " ").trim();
}
export function MessageMinimap({ groups, onSelect }: Props) {
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);
  const userGroups = groups.map((group, index) => ({ group, index })).filter(({ group }) => group.role === "user");
  if (userGroups.length === 0) return null;
  return (
    <nav className="message-minimap" aria-label="User message navigation">
      {userGroups.map(({ group, index }) => {
        const weight = groupWeight(group);
        const width = Math.min(44, Math.max(12, 12 + Math.round(weight / 18)));
        const top = groups.length <= 1 ? 0 : (index / (groups.length - 1)) * 100;
        const preview = groupPreview(group) || "User message";
        return (
          <button key={`${group.key}-${index}`} type="button" className="message-minimap-marker" style={{ top: `${top}%`, width: `${width}px` }} onClick={() => onSelect(index)} onMouseEnter={() => setHoveredIndex(index)} onMouseLeave={() => setHoveredIndex(null)} aria-label={`Jump to user message ${userGroups.findIndex((entry) => entry.index === index) + 1}: ${preview.slice(0, 120)}`}>
            {hoveredIndex === index && <span className="message-minimap-tooltip">{preview}</span>}
          </button>
        );
      })}
    </nav>
  );
}
