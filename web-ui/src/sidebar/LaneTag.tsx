import React from "react";
import { SquareKanban } from "lucide-react";
import type { SessionTaskLink } from "./useSessionTaskLinks";

/** Small badge on a sidebar session row marking it as launched from a kanban
 *  task, showing its current lane. Tinted with the lane's colour (falling back
 *  to the accent colour) so it reads in both glassy and flat themes. */
export const LaneTag = React.memo(function LaneTag({ link }: { link: SessionTaskLink }) {
  const style = link.laneColor
    ? ({ "--lane-color": link.laneColor } as React.CSSProperties)
    : undefined;
  return (
    <span
      className="sb-lane-tag"
      style={style}
      title={`Kanban · ${link.laneName}`}
    >
      <SquareKanban size={9} />
      {link.laneName}
    </span>
  );
});
