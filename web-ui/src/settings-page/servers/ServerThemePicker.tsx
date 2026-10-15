import React, { useEffect, useState } from "react";
import { Palette } from "lucide-react";
import { SERVER_ID } from "../../api/base";
import { fetchServerTheme, setServerTheme } from "../../api/serverTheme";
import { applyThemeToCss } from "../../utils/theme";
import { getPersistedAppearance, resolveThemeColors, storeThemePair } from "../../utils/appearance";

export interface ServerThemePickerProps {
  readonly serverId: string;
  readonly serverName: string;
  /** Every theme name on offer, sorted. */
  readonly themes: readonly string[];
}

/**
 * One server's theme. The server keeps it, so every pane showing that server's content
 * paints with it — home's panes in one palette, a relayed remote's in another. A change
 * reaches open panes through that server's own `theme_changed` event; the page that
 * made it applies it at once rather than waiting for the round trip.
 */
export function ServerThemePicker({ serverId, serverName, themes }: ServerThemePickerProps) {
  const [current, setCurrent] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let live = true;
    fetchServerTheme(serverId)
      .then((pair) => live && setCurrent(pair.name))
      .catch(() => live && setFailed(true));
    return () => {
      live = false;
    };
  }, [serverId]);

  const choose = (name: string) => {
    const previous = current;
    setCurrent(name);
    setServerTheme(serverId, name)
      .then((pair) => {
        if (serverId !== SERVER_ID) return;
        const appearance = getPersistedAppearance();
        storeThemePair(pair, appearance);
        applyThemeToCss(resolveThemeColors(pair, appearance));
      })
      .catch(() => setCurrent(previous));
  };

  if (failed) return null;
  return (
    <label className="srv-theme" title={`Theme for panes showing ${serverName}`}>
      <Palette size={13} aria-hidden="true" />
      <select
        className="stg-input srv-theme-select"
        aria-label={`Theme for ${serverName}`}
        value={current ?? ""}
        disabled={current === null}
        onChange={(event) => choose(event.target.value)}
      >
        {current === null && <option value="">Loading…</option>}
        {themes.map((name) => (
          <option key={name} value={name}>{name}</option>
        ))}
      </select>
    </label>
  );
}
