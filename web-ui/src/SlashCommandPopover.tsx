import React, { useState, useEffect, useMemo } from "react";
import { fetchCommands } from "./api";
import { OPMAN_SLASH_COMMANDS } from "./keybindings/commands";
import { useKeymapContext } from "./keybindings/KeymapContext";
import { evaluateWhen } from "./keybindings/when";
import type { SlashCommand } from "./types";

interface Props {
  filter: string;
  onSelect: (command: SlashCommand) => void;
  onClose: () => void;
  sessionId: string | null;
  /** Runner serving this session — the one whose commands are offered. */
  runner?: string;
}

/**
 * The slash command list.
 *
 * Two sources, and neither is a table in this file. opman's own commands come from the
 * command registry, so a `/name` cannot exist without also being in the palette, the
 * cheatsheet and the keybindings view — and cannot rot when the surface behind it is
 * renamed or removed. Everything the *agent* runs comes from the agent: opencode's command
 * API, claude's `slash_commands`, an ACP agent's `available_commands_update`. Whichever
 * runner is serving the session answers for itself, which is the only way a command opman
 * has never heard of can be offered at all.
 */
export function SlashCommandPopover({ filter, onSelect, onClose, sessionId, runner }: Props) {
  const [runnerCommands, setRunnerCommands] = useState<SlashCommand[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const { context, isRunnable } = useKeymapContext();

  useEffect(() => {
    let live = true;
    fetchCommands(runner)
      .then((cmds) => {
        // Not `if (cmds.length)`: an empty answer is an answer. A runner that advertises
        // nothing must clear whatever the previous runner advertised, or its commands
        // linger in the list and fail on send.
        if (live) setRunnerCommands(cmds);
      })
      .catch(() => {
        if (live) setRunnerCommands([]);
      });
    return () => {
      live = false;
    };
  }, [runner, sessionId]);

  /**
   * opman's commands, minus the ones that cannot run right now.
   *
   * A command is offered only when a mounted surface has registered a handler for it and
   * its `when` clause holds — the same two tests the keymap applies to a chord. Listing
   * `/todos` with no session, and having it do nothing, is the failure this avoids.
   */
  const localCommands = useMemo(
    () =>
      OPMAN_SLASH_COMMANDS.filter(
        (command) => isRunnable(command.id) && evaluateWhen(command.when, context),
      ).map<SlashCommand>((command) => ({
        name: command.slash.name,
        description: command.title,
      })),
    [context, isRunnable],
  );

  // opman's own names win a collision: `/new` opens a session here rather than being
  // forwarded to an agent that happens to use the same word for something else.
  const commands = useMemo(() => {
    const taken = new Set(localCommands.map((c) => c.name));
    return [...localCommands, ...runnerCommands.filter((c) => !taken.has(c.name))];
  }, [localCommands, runnerCommands]);

  const filtered = useMemo(() => {
    if (!filter) return commands;
    const lf = filter.toLowerCase();
    return commands.filter(
      (c) =>
        c.name.toLowerCase().includes(lf) ||
        c.description?.toLowerCase().includes(lf)
    );
  }, [commands, filter]);

  // Reset selection when filter changes
  useEffect(() => {
    setSelectedIndex(0);
  }, [filter]);

  useEffect(() => {
    /**
     * While the list is open it owns these keys outright.
     *
     * Stopping propagation is the point: the listener is on `document` in capture, so
     * without it the same Enter continued to the textarea and the composer submitted the
     * *typed prefix* as well. Picking `/agent` from a half-typed `/ag` therefore ran the
     * command and forwarded `/ag` to the agent, which — being a name no runner knows —
     * arrived as a plain message.
     */
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "ArrowDown") {
        setSelectedIndex((i) => Math.min(i + 1, filtered.length - 1));
      } else if (e.key === "ArrowUp") {
        setSelectedIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === "Enter" || e.key === "Tab") {
        const command = filtered[selectedIndex];
        if (!command) return;
        onSelect(command);
      } else if (e.key === "Escape") {
        onClose();
      } else {
        return;
      }
      e.preventDefault();
      e.stopPropagation();
      e.stopImmediatePropagation();
    }
    document.addEventListener("keydown", handleKeyDown, true);
    return () => document.removeEventListener("keydown", handleKeyDown, true);
  }, [filtered, selectedIndex, onSelect, onClose]);

  if (filtered.length === 0) return null;

  return (
    <div className="slash-popover composer-popover" role="listbox" aria-label="Slash commands">
      <div className="composer-popover-group">Commands</div>
      {filtered.map((cmd, idx) => (
        <button
          key={cmd.name}
          type="button"
          role="option"
          aria-selected={idx === selectedIndex}
          className={`composer-popover-row${idx === selectedIndex ? " is-cursor" : ""}`}
          /* Selected on press, not on click: a tap here blurs the textarea, and the
             on-screen keyboard collapsing between touchstart and touchend moves the
             composer up under the finger — so the click landed on the send button
             instead of the row, and the half-typed slash went out as a message. */
          onPointerDown={(e) => { e.preventDefault(); onSelect(cmd); }}
          onMouseEnter={() => setSelectedIndex(idx)}
        >
          {/* The slash belongs to the name: this is literally what gets typed,
              and a repeated glyph in an icon column would only be decoration. */}
          <span className="composer-popover-key">/{cmd.name}</span>
          {cmd.description && (
            <span className="composer-popover-detail">{cmd.description}</span>
          )}
          {cmd.args && (
            <span className="composer-popover-meta">{cmd.args}</span>
          )}
        </button>
      ))}
    </div>
  );
}
