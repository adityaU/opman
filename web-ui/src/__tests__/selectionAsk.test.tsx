/**
 * Selecting text in an assistant reply offers "Ask" (send a question about it right
 * away, as a user message) and "Quote" (put it in the composer). Selections outside an
 * assistant reply, or across two replies, offer nothing.
 */
import React, { useRef } from "react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { SelectionAsk } from "../selection-ask/SelectionAsk";
import { askMessage, insertIntoComposer, MAX_QUOTE_CHARS, quote } from "../selection-ask/quote";
import { useComposerInsert } from "../prompt-input/useComposerInsert";

beforeAll(() => {
  // jsdom lays nothing out; a selection needs a box for the bar to sit next to.
  Range.prototype.getBoundingClientRect = () => new DOMRect(100, 200, 80, 16);
});
afterEach(() => {
  cleanup();
  window.getSelection()?.removeAllRanges();
});

function Timeline({ onSend }: { onSend?: (text: string) => void }) {
  const ref = useRef<HTMLDivElement>(null);
  return (
    <div ref={ref}>
      <div className="message-turn message-turn-user"><p>user words</p></div>
      <div className="message-turn message-turn-assistant"><p>The cache is keyed by path.</p></div>
      <div className="message-turn message-turn-assistant"><p>Second reply.</p></div>
      <SelectionAsk containerRef={ref} sessionId="s1" onSend={onSend} />
    </div>
  );
}

function select(text: string, until?: string) {
  const start = screen.getByText(text).firstChild as Text;
  const end = until ? (screen.getByText(until).firstChild as Text) : start;
  const range = document.createRange();
  range.setStart(start, 0);
  range.setEnd(end, end.length);
  const selection = window.getSelection()!;
  selection.removeAllRanges();
  selection.addRange(range);
  fireEvent.mouseUp(start.parentElement!);
  act(() => vi.runAllTimers());
}

describe("quote", () => {
  it("quotes every line and keeps blank ones", () => {
    expect(quote("a\n\nb")).toBe("> a\n>\n> b");
    expect(askMessage("x", " why? ")).toBe("> x\n\nwhy?");
  });

  it("cuts a very long selection", () => {
    expect(quote("x".repeat(MAX_QUOTE_CHARS + 50)).length).toBe(MAX_QUOTE_CHARS + 3);
  });
});

describe("SelectionAsk", () => {
  it("asks about the selected text as a user message", () => {
    vi.useFakeTimers();
    const onSend = vi.fn();
    render(<Timeline onSend={onSend} />);
    select("The cache is keyed by path.");
    fireEvent.click(screen.getByRole("button", { name: /Ask/ }));
    fireEvent.change(screen.getByLabelText("Question about the selected text"), {
      target: { value: "What about symlinks?" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Send question" }));
    expect(onSend).toHaveBeenCalledWith("> The cache is keyed by path.\n\nWhat about symlinks?");
    expect(screen.queryByRole("toolbar")).toBeNull();
    vi.useRealTimers();
  });

  it("offers nothing for a user message or a selection across two replies", () => {
    vi.useFakeTimers();
    render(<Timeline onSend={vi.fn()} />);
    select("user words");
    expect(screen.queryByRole("toolbar")).toBeNull();
    select("The cache is keyed by path.", "Second reply.");
    expect(screen.queryByRole("toolbar")).toBeNull();
    vi.useRealTimers();
  });

  it("quotes into the composer of the same session", () => {
    vi.useFakeTimers();
    const heard = vi.fn();
    window.addEventListener("opman:composer-insert", heard);
    render(<Timeline />);
    select("Second reply.");
    // Without a send hook only "Quote" is offered.
    expect(screen.queryByRole("button", { name: /Ask/ })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /Quote/ }));
    expect((heard.mock.calls[0][0] as CustomEvent).detail).toEqual({ sessionId: "s1", text: "> Second reply.\n\n" });
    window.removeEventListener("opman:composer-insert", heard);
    vi.useRealTimers();
  });
});

describe("useComposerInsert", () => {
  it("appends text addressed to its own session and ignores others", () => {
    let text = "draft";
    const setText = (update: (current: string) => string) => { text = update(text); };
    const textarea = { current: document.createElement("textarea") };
    renderHook(() => useComposerInsert("s1", setText, textarea));
    act(() => insertIntoComposer("other", "> no\n\n"));
    expect(text).toBe("draft");
    act(() => insertIntoComposer("s1", "> yes\n\n"));
    expect(text).toBe("draft\n\n> yes\n\n");
  });
});
