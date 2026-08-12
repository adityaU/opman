import { describe, expect, it } from "vitest";
import { applyCmdlineShow, createCmdlineState } from "../state/cmdline";
import { createMessageState, applyMessageShow, showMessageHistory } from "../state/messages";
import { applyModeChange, applyModeInfo, createModeState, cursorInvertsCell } from "../state/modes";
import { createOptionState, setOption, setTitle, setMouse, ringBell, setBusy } from "../state/options";
import { createPopupmenuState, showPopupmenu, selectPopupmenu, hidePopupmenu } from "../state/popupmenu";
import { createTablineState, updateTabline } from "../state/tabline";

describe("Neovim overlay reducers", () => {
  it("tracks cmdline content, mode metadata, and messages", () => {
    const cmdline = createCmdlineState();
    applyCmdlineShow(cmdline, [[1, ":"], [2, "x", 2]], 2, ":", "", 0);
    expect(cmdline.visible).toBe(true);
    expect(cmdline.content).toHaveLength(3);
    const messages = createMessageState();
    applyMessageShow(messages, "emsg", [[3, "bad"]], false, true);
    showMessageHistory(messages, [["echo", [[4, "old"]]]]);
    expect(messages.history[0].kind).toBe("echo");
    const modes = createModeState();
    applyModeInfo(modes, true, [{ name: "normal", shortName: "N", cursorShape: "block", cellPercentage: 100, attrId: 0, blinkWait: 0, blinkOn: 0, blinkOff: 0, conceal: "", canInsert: false, canUndo: true }]);
    applyModeChange(modes, "normal", 0);
    expect(cursorInvertsCell(modes)).toBe(true);
  });

  it("handles options, popupmenu, and tabline state", () => {
    const options = createOptionState();
    setOption(options, "guifont", "Iosevka");
    setOption(options, "linespace", 2);
    setOption(options, "rgb", false);
    setTitle(options, "opman"); setMouse(options, false); ringBell(options); setBusy(options, true);
    expect(options).toMatchObject({ guifont: "Iosevka", linespace: 2, title: "opman", mouseEnabled: false, bellCount: 1, busy: true });
    const popup = createPopupmenuState();
    showPopupmenu(popup, [{ word: "one", abbr: "1", menu: "", info: "", kind: "f", icase: false, dup: false, empty: false }], 0, 3, 4);
    selectPopupmenu(popup, -1); hidePopupmenu(popup);
    expect(popup.visible).toBe(false);
    const tabline = createTablineState();
    updateTabline(tabline, 1, [{ tab: "1", name: "main", buffer: 1 }], 1);
    expect(tabline.currentTab).toBe("1");
  });
});
