import { describe, expect, it } from "vitest";
import {
  applyMessageShow, clearMessages, createMessageState, setMessageCommand, setMessageMode,
  setMessagePosition, setMessageRuler, showMessageHistory,
} from "../state/messages";

describe("Neovim message reducer", () => {
  it("retains msg_showmode content when ext_messages owns the grid", () => {
    const state = createMessageState();
    setMessageMode(state, [[15, "-- INSERT --", 2]]);

    expect(state.mode.map((cell) => cell.text)).toEqual(["-- INSERT --", "-- INSERT --"]);
  });

  it("retains msg_showcmd and msg_ruler independently", () => {
    const state = createMessageState();
    setMessageCommand(state, [[1, "2/10"]]);
    setMessageRuler(state, [[2, "10,4"]]);

    expect(state.command.map((cell) => [cell.hlId, cell.text])).toEqual([[1, "2/10"]]);
    expect(state.ruler.map((cell) => [cell.hlId, cell.text])).toEqual([[2, "10,4"]]);
  });

  it("replaces the visible message and preserves history entries", () => {
    const state = createMessageState();
    applyMessageShow(state, "search_count", [[3, "[3/8]"]], false, true);
    applyMessageShow(state, "search_count", [[4, "[4/8]"]], true, false);

    expect(state.items).toHaveLength(1);
    expect(state.items[0]).toMatchObject({ kind: "search_count", replaceLast: true, history: false });
    expect(state.items[0].content[0].text).toBe("[4/8]");
    expect(state.history.map((item) => item.content[0].text)).toEqual(["[3/8]"]);
  });

  it("loads msg_history_show and tracks position, scrolling, and clearing", () => {
    const state = createMessageState();
    showMessageHistory(state, [
      ["echo", [[1, "old"]]],
      ["emsg", [[2, "error", 2]]],
    ]);
    setMessagePosition(state, 7, true, "---");

    expect(state.items).toHaveLength(2);
    expect(state.history[1].content.map((cell) => cell.text)).toEqual(["error", "error"]);
    expect(state.position).toBe(7);
    expect(state.scrolled).toBe(true);
    expect(state.separator).toBe("---");

    clearMessages(state);
    expect(state.items).toHaveLength(0);
    expect(state.history).toHaveLength(2);
  });
});
