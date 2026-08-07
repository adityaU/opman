import { describe, expect, it } from "vitest";
import { COMMANDS } from "../keybindings/commands";
import { validate } from "../keybindings/conflicts";
import { configLayer, DEFAULT_CONFIG, parseConfig, userLayer } from "../keybindings/config";
import { builtInLayers } from "../keybindings/layers";
import { resolve } from "../keybindings/resolve";
import type { Host } from "../keybindings/types";

const HOST: Host = { platform: "linux", target: "web", browser: "chrome" };

function compose(config = DEFAULT_CONFIG, user: Parameters<typeof userLayer>[0] = []) {
  return resolve([...builtInLayers(), configLayer(config), userLayer(user)], {
    host: HOST,
    mode: config.mode,
    leader: config.leader,
    localLeader: config.localLeader,
  });
}

describe("parseConfig", () => {
  it("fills every field from an empty object", () => {
    expect(parseConfig({}).config).toEqual(DEFAULT_CONFIG);
  });

  it("falls back to defaults for a non-object", () => {
    expect(parseConfig(null).config).toEqual(DEFAULT_CONFIG);
    expect(parseConfig("nope").config).toEqual(DEFAULT_CONFIG);
  });

  it("keeps valid scalars and rejects unknown enum values", () => {
    const { config } = parseConfig({ mode: "vim", leader: ",", chordTimeoutMs: 900 });
    expect(config).toMatchObject({ mode: "vim", leader: ",", chordTimeoutMs: 900 });

    expect(parseConfig({ mode: "emacs" }).config.mode).toBe("normal");
    expect(parseConfig({ chordTimeoutMs: -1 }).config.chordTimeoutMs).toBe(1500);
  });

  it("drops a malformed binding and reports it, keeping the rest", () => {
    const { config, diagnostics } = parseConfig({
      bindings: [
        { key: "ctrl+b", command: "layout.toggleSidebar" },
        { command: "session.new" },
        "not an object",
        { key: "ctrl+j" },
      ],
    });

    expect(config.bindings).toHaveLength(1);
    expect(diagnostics.map((d) => d.message)).toEqual([
      'bindings[1] is missing "key"',
      "bindings[2] is not an object",
      'bindings[3] is missing "command"',
    ]);
  });

  it("strips scope fields it does not recognize", () => {
    const { config } = parseConfig({
      bindings: [{ key: "ctrl+b", command: "x", platform: "solaris", browser: "firefox" }],
    });
    expect(config.bindings[0]).toMatchObject({ platform: undefined, browser: "firefox" });
  });
});

describe("config layer", () => {
  it("changes nothing when empty", () => {
    const withConfig = compose().bindings.map((b) => `${b.id}:${b.command}`);
    const builtIn = resolve(builtInLayers(), { host: HOST, mode: "normal" }).bindings.map(
      (b) => `${b.id}:${b.command}`,
    );
    expect(withConfig).toEqual(builtIn);
  });

  it("adds a user chord alongside the default rather than replacing it", () => {
    const config = {
      ...DEFAULT_CONFIG,
      bindings: [{ key: "ctrl+alt+b", command: "layout.toggleSidebar" }],
    };
    const chords = compose(config)
      .bindings.filter((b) => b.command === "layout.toggleSidebar")
      .map((b) => b.id);
    expect(chords).toEqual(expect.arrayContaining(["ctrl+b", "ctrl+alt+b"]));
  });

  it("removes a default with a leading minus", () => {
    const config = {
      ...DEFAULT_CONFIG,
      bindings: [{ key: "ctrl+b", command: "-layout.toggleSidebar" }],
    };
    const chords = compose(config)
      .bindings.filter((b) => b.command === "layout.toggleSidebar")
      .map((b) => b.id);
    expect(chords).toEqual([]);
  });

  it("clears every binding with -*", () => {
    const config = { ...DEFAULT_CONFIG, bindings: [{ key: "*", command: "-*" }] };
    expect(compose(config).bindings).toHaveLength(0);
  });

  it("reports a malformed chord without dropping the other entries", () => {
    const config = {
      ...DEFAULT_CONFIG,
      bindings: [
        { key: "hyper+b", command: "layout.toggleSidebar" },
        { key: "ctrl+alt+b", command: "layout.toggleSidebar" },
      ],
    };
    const { bindings, rejected } = compose(config);
    expect(rejected).toHaveLength(1);
    expect(rejected[0].kind).toBe("malformed");
    expect(bindings.some((b) => b.id === "ctrl+alt+b")).toBe(true);
  });

  it("flags a binding pointing at a command that does not exist", () => {
    const config = {
      ...DEFAULT_CONFIG,
      bindings: [{ key: "ctrl+alt+9", command: "session.explode" }],
    };
    const { bindings } = compose(config);
    const conflicts = validate({ bindings, host: HOST, commands: COMMANDS });
    expect(conflicts.map((c) => c.kind)).toContain("unknown-command");
  });

  it("honours a custom leader", () => {
    const config = { ...DEFAULT_CONFIG, mode: "vim" as const, leader: "," };
    const gitToggle = compose(config).bindings.find(
      (b) => b.command === "layout.toggleGit" && b.mode === "vim",
    );
    expect(gitToggle?.id).toBe(", g g");
  });

  it("lets the user layer win over the config layer", () => {
    const config = {
      ...DEFAULT_CONFIG,
      bindings: [{ key: "ctrl+b", command: "-layout.toggleSidebar" }],
    };
    const chords = compose(config, [{ key: "ctrl+alt+b", command: "layout.toggleSidebar" }])
      .bindings.filter((b) => b.command === "layout.toggleSidebar")
      .map((b) => b.id);
    expect(chords).toEqual(["ctrl+alt+b"]);
  });
});
