import { useMemo } from "react";
import { displayStep } from "../keybindings/chord";
import { commandLabel, findCommand } from "../keybindings/commands";
import type { Keymap } from "../keybindings/matcher";
import type { Host, ResolvedBinding } from "../keybindings/types";

/**
 * The vim leader tree, shown exactly as which-key will render it.
 *
 * Also lists the keys still free in each namespace — the question anyone
 * rebinding actually has is "what can I use", and answering it here saves a
 * round trip through the capture dialog's conflict warning.
 */

const ALPHABET = "abcdefghijklmnopqrstuvwxyz".split("");

export interface LeaderTreeProps {
  readonly keymap: Keymap;
  readonly host: Host;
  readonly leaderLabel: string;
}

interface Namespace {
  readonly group: string;
  readonly prefixKey: string;
  readonly leaves: readonly ResolvedBinding[];
  readonly free: readonly string[];
}

function labelFor(binding: ResolvedBinding): string {
  if (binding.label) return binding.label;
  const command = findCommand(binding.command);
  return command ? commandLabel(command) : binding.command;
}

function buildNamespaces(keymap: Keymap, host: Host): Namespace[] {
  const byPrefix = new Map<string, ResolvedBinding[]>();
  const direct: ResolvedBinding[] = [];

  for (const binding of keymap.all) {
    if (binding.mode !== "vim") continue;
    // A leader chord is three steps: leader, namespace, leaf. Two steps is a
    // leaf hanging directly off the leader.
    if (binding.seq.length === 3) {
      const key = displayStep(binding.seq[1], host.platform, "vim");
      const bucket = byPrefix.get(key);
      if (bucket) bucket.push(binding);
      else byPrefix.set(key, [binding]);
      continue;
    }
    if (binding.seq.length === 2) direct.push(binding);
  }

  const namespaces = [...byPrefix.entries()].map(([prefixKey, leaves]) => {
    const used = new Set(leaves.map((b) => b.seq[2].key));
    return {
      group: leaves[0].group ?? prefixKey,
      prefixKey,
      leaves: [...leaves].sort((a, b) => a.seq[2].key.localeCompare(b.seq[2].key)),
      free: ALPHABET.filter((letter) => !used.has(letter)),
    };
  });

  if (direct.length > 0) {
    namespaces.unshift({
      group: "leader",
      prefixKey: "",
      leaves: direct,
      free: [],
    });
  }

  return namespaces.sort((a, b) => a.group.localeCompare(b.group));
}

export function LeaderTree({ keymap, host, leaderLabel }: LeaderTreeProps) {
  const namespaces = useMemo(() => buildNamespaces(keymap, host), [keymap, host]);

  return (
    <div className="kbv-tree">
      {namespaces.map((namespace) => (
        <details className="kbv-ns" key={namespace.group} open>
          <summary className="kbv-ns-head">
            <span className="kbv-ns-key">
              {leaderLabel}
              {namespace.prefixKey ? ` ${namespace.prefixKey}` : ""}
            </span>
            <span className="kbv-ns-name">+{namespace.group}</span>
            <span className="kbv-ns-count">{namespace.leaves.length}</span>
          </summary>
          <ul className="kbv-ns-leaves">
            {namespace.leaves.map((leaf) => (
              <li className="kbv-leaf" key={leaf.id}>
                <kbd className="kbv-chip">
                  {displayStep(leaf.seq[leaf.seq.length - 1], host.platform, "vim")}
                </kbd>
                <span className="kbv-leaf-label">{labelFor(leaf)}</span>
                <code className="kbv-leaf-command">{leaf.command}</code>
              </li>
            ))}
          </ul>
          {namespace.free.length > 0 ? (
            <p className="kbv-ns-free">
              Free: <code>{namespace.free.join(" ")}</code>
            </p>
          ) : null}
        </details>
      ))}
    </div>
  );
}
