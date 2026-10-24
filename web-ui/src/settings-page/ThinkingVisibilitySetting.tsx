import React from "react";
import { useThinkingVisibility, saveThinkingVisibility } from "../utils/thinkingVisibility";

export function ThinkingVisibilitySetting() {
  const showThinking = useThinkingVisibility();

  return (
    <label className="stg-row stg-thinking-row">
      <span className="stg-row-main">
        <span className="stg-row-name">Show thinking</span>
        <span className="stg-row-origin">
          Show reasoning headings and text in assistant messages on this device.
        </span>
      </span>
      <input
        className="stg-thinking-checkbox"
        type="checkbox"
        checked={showThinking}
        onChange={(event) => saveThinkingVisibility(event.currentTarget.checked)}
        aria-label="Show thinking"
      />
    </label>
  );
}
