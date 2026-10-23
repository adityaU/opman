// Evaluated once per run with the host bridge as its only argument. It builds the
// script-facing globals and returns the function that prints the final value.
(function (host) {
  "use strict";
  const replacer = (_key, value) => (typeof value === "bigint" ? value.toString() + "n" : value);

  const show = (value) => {
    if (typeof value === "string") return value;
    if (value === undefined) return "undefined";
    if (value instanceof Error) return value.name + ": " + value.message;
    try {
      const text = JSON.stringify(value, replacer);
      return text === undefined ? String(value) : text;
    } catch (_) {
      return String(value);
    }
  };
  const logger = (level) => (...args) => host.log(level, args.map(show).join(" "));
  const console = Object.freeze({
    log: logger("log"),
    info: logger("info"),
    warn: logger("warn"),
    error: logger("error"),
    debug: logger("debug"),
  });

  const TOOL_ALIASES = Object.freeze({
    read: "read_file",
    view_file: "read_file",
    write: "write_file",
    create_file: "write_file",
    edit: "edit_file",
    replace_file_content: "edit_file",
    ls: "list_dir",
    list_directory: "list_dir",
    find: "glob",
    find_files: "glob",
    search: "grep",
    grep_search: "grep",
    exec: "bash",
    run: "bash",
    run_command: "bash",
    sh: "bash",
    fetch: "web_fetch",
  });

  const normalizeInput = (tool, args) => {
    const first = args[0];
    if (args.length === 0 || first === undefined) return {};
    if (first !== null && typeof first === "object" && !Array.isArray(first)) return first;
    if (typeof first === "string") {
      switch (tool) {
        case "read_file": {
          const out = { path: first };
          if (typeof args[1] === "number") out.offset = args[1];
          if (typeof args[2] === "number") out.limit = args[2];
          else if (args[1] && typeof args[1] === "object") Object.assign(out, args[1]);
          return out;
        }
        case "write_file":
          return {
            path: first,
            content: args[1] === undefined ? "" : String(args[1]),
            ...(typeof args[2] === "boolean" ? { append: args[2] } : (args[2] && typeof args[2] === "object" ? args[2] : {})),
          };
        case "edit_file":
          if (Array.isArray(args[1])) return { path: first, edits: args[1] };
          if (args[1] && typeof args[1] === "object") return { path: first, ...args[1] };
          return {
            path: first,
            old_string: args[1] === undefined ? "" : String(args[1]),
            new_string: args[2] === undefined ? "" : String(args[2]),
            ...(typeof args[3] === "boolean" ? { replace_all: args[3] } : {}),
          };
        case "bash":
          return {
            command: first,
            ...(args[1] && typeof args[1] === "object" ? args[1] : (typeof args[1] === "string" ? { cwd: args[1] } : {})),
          };
        case "glob":
          return {
            pattern: first,
            ...(typeof args[1] === "string" ? { path: args[1] } : (args[1] && typeof args[1] === "object" ? args[1] : {})),
          };
        case "grep":
          return {
            pattern: first,
            ...(typeof args[1] === "string" ? { path: args[1] } : (args[1] && typeof args[1] === "object" ? args[1] : {})),
          };
        case "list_dir":
          return { path: first };
        case "web_fetch":
          return {
            url: first,
            ...(typeof args[1] === "number" ? { max_bytes: args[1] } : (args[1] && typeof args[1] === "object" ? args[1] : {})),
          };
        case "search":
          return { query: first };
      }
    }
    if (typeof first === "number" && tool === "wait") {
      return { seconds: first };
    }
    return first;
  };

  const defineHidden = (obj, props) => {
    for (const [k, desc] of Object.entries(props)) {
      if (!(k in obj)) {
        Object.defineProperty(obj, k, { ...desc, enumerable: false, configurable: true });
      }
    }
  };

  const enrichOutput = (val) => {
    if (!val || typeof val !== "object" || Array.isArray(val)) return val;
    if (typeof val.content === "string") {
      defineHidden(val, {
        toString: { value() { return this.content; } },
        valueOf: { value() { return this.content; } },
        split: { value(...a) { return this.content.split(...a); } },
        includes: { value(...a) { return this.content.includes(...a); } },
        replace: { value(...a) { return this.content.replace(...a); } },
        replaceAll: { value(...a) { return this.content.replaceAll(...a); } },
        match: { value(...a) { return this.content.match(...a); } },
        matchAll: { value(...a) { return this.content.matchAll(...a); } },
        trim: { value() { return this.content.trim(); } },
        slice: { value(...a) { return this.content.slice(...a); } },
        indexOf: { value(...a) { return this.content.indexOf(...a); } },
        startsWith: { value(...a) { return this.content.startsWith(...a); } },
        endsWith: { value(...a) { return this.content.endsWith(...a); } },
        lines: { get() { return this.content.split(/\r?\n/); } },
      });
    } else if (typeof val.stdout === "string") {
      defineHidden(val, {
        toString: { value() { return this.stdout; } },
        valueOf: { value() { return this.stdout; } },
        trim: { value() { return this.stdout.trim(); } },
        split: { value(...a) { return this.stdout.split(...a); } },
        includes: { value(...a) { return this.stdout.includes(...a); } },
        match: { value(...a) { return this.stdout.match(...a); } },
        replace: { value(...a) { return this.stdout.replace(...a); } },
        lines: { get() { return this.stdout.trimEnd().split(/\r?\n/); } },
        output: { get() { return this.stdout; } },
        exitCode: { get() { return this.exit_code; } },
      });
    }
    const list = Array.isArray(val.files)
      ? val.files
      : Array.isArray(val.results)
      ? val.results
      : Array.isArray(val.entries)
      ? val.entries
      : null;
    if (list) {
      defineHidden(val, {
        [Symbol.iterator]: { value() { return list[Symbol.iterator](); } },
        map: { value(...a) { return list.map(...a); } },
        filter: { value(...a) { return list.filter(...a); } },
        forEach: { value(...a) { return list.forEach(...a); } },
        find: { value(...a) { return list.find(...a); } },
        some: { value(...a) { return list.some(...a); } },
        every: { value(...a) { return list.every(...a); } },
        slice: { value(...a) { return list.slice(...a); } },
        flatMap: { value(...a) { return list.flatMap(...a); } },
        join: { value(...a) { return list.join(...a); } },
        includes: { value(...a) { return list.includes(...a); } },
        length: { get() { return list.length; } },
      });
    }
    return val;
  };

  const rawGroups = Object.create(null);

  const makeCompoundHelpers = (opmanGroup) => ({
    read_files: async (input) => {
      const paths = Array.isArray(input) ? input : (input && Array.isArray(input.paths) ? input.paths : []);
      const pairs = await Promise.all(
        paths.map(async (p) => {
          const res = await opmanGroup.read_file({ path: String(p) });
          return [String(p), res];
        })
      );
      return Object.fromEntries(pairs);
    },
    multi_edit: async (pathOrInput, maybeEdits) => {
      if (typeof pathOrInput === "string") {
        return opmanGroup.edit_file({ path: pathOrInput, edits: maybeEdits });
      }
      return opmanGroup.edit_file(pathOrInput);
    },
  });

  // Reading a name that does not exist fails loudly with a suggestion, instead of
  // returning undefined and failing later with "x is not a function".
  const guarded = (target, label, extraResolver) =>
    new Proxy(target, {
      get(object, key, receiver) {
        if (key in object) return Reflect.get(object, key, receiver);
        if (typeof key !== "string" || key === "then" || key === "toJSON") return undefined;
        if (extraResolver) {
          const resolved = extraResolver(key);
          if (resolved !== undefined) return resolved;
        }
        throw new Error(host.unknown(label, key));
      },
      set() {
        throw new TypeError("tools is read-only");
      },
      deleteProperty() {
        throw new TypeError("tools is read-only");
      },
    });

  const tools = {};
  for (const { ns, tools: names } of JSON.parse(host.namespaces())) {
    const group = {};
    for (const name of names) {
      group[name] = async (...args) => {
        const caller = new Error();
        const normalized = normalizeInput(name, args);
        const raw = await host.call(ns, name, JSON.stringify(normalized === undefined ? {} : normalized));
        const reply = JSON.parse(raw);
        if (!reply.ok) {
          const error = new Error(reply.error);
          error.stack = caller.stack;
          throw error;
        }
        return enrichOutput(reply.value);
      };
    }
    rawGroups[ns] = group;
    const helpers = ns === "opman_tools" ? makeCompoundHelpers(group) : null;
    tools[ns] = guarded(group, ns, (key) => {
      const mapped = TOOL_ALIASES[key];
      if (mapped && mapped in group) return group[mapped];
      if (helpers && key in helpers) return helpers[key];
      return undefined;
    });
  }
  tools.$codemode = guarded(
    {
      search: async (...args) => {
        const input = normalizeInput("search", args);
        return JSON.parse(host.search(String((input && input.query) || "")));
      },
    },
    "$codemode"
  );

  const rootHelpers = rawGroups.opman_tools ? makeCompoundHelpers(rawGroups.opman_tools) : null;
  const resolveRootShorthand = (key) => {
    if ((key === "opman" || key === "coding" || key === "fs") && tools.opman_tools) {
      return tools.opman_tools;
    }
    const opman = rawGroups.opman_tools;
    if (opman) {
      if (key in opman) return opman[key];
      const mapped = TOOL_ALIASES[key];
      if (mapped && mapped in opman) return opman[mapped];
      if (rootHelpers && key in rootHelpers) return rootHelpers[key];
    }
    return undefined;
  };

  let timer = 0;
  const cleared = new Set();
  const setTimeout = (fn, ms, ...args) => {
    const id = ++timer;
    host.sleep(Number(ms) || 0).then(() => {
      if (!cleared.delete(id)) fn(...args);
    });
    return id;
  };
  const clearTimeout = (id) => void cleared.add(id);

  const define = (name, value) =>
    Object.defineProperty(globalThis, name, { value, writable: false, configurable: false });
  define("tools", guarded(tools, "", resolveRootShorthand));
  define("console", console);
  define("setTimeout", setTimeout);
  define("clearTimeout", clearTimeout);

  return (value) => {
    if (value === undefined) return "";
    if (typeof value === "string") return value;
    try {
      const text = JSON.stringify(value, replacer, 2);
      return text === undefined ? String(value) : text;
    } catch (error) {
      return "[value could not be printed: " + error.message + "]";
    }
  };
})
