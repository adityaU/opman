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

  // Reading a name that does not exist fails loudly with a suggestion, instead of
  // returning undefined and failing later with "x is not a function".
  const guarded = (target, label) =>
    new Proxy(target, {
      get(object, key, receiver) {
        if (key in object) return Reflect.get(object, key, receiver);
        if (typeof key !== "string" || key === "then" || key === "toJSON") return undefined;
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
      group[name] = async (input) => {
        const caller = new Error();
        const raw = await host.call(ns, name, JSON.stringify(input === undefined ? {} : input));
        const reply = JSON.parse(raw);
        if (!reply.ok) {
          const error = new Error(reply.error);
          error.stack = caller.stack;
          throw error;
        }
        return reply.value;
      };
    }
    tools[ns] = guarded(group, ns);
  }
  tools.$codemode = guarded(
    { search: async (input) => JSON.parse(host.search(String((input && input.query) || ""))) },
    "$codemode"
  );

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
  define("tools", guarded(tools, ""));
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
