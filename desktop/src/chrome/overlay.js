// Title bar overlay injected into every page of the main window (macOS + Windows).
// Rust decides when the bar is shown (it polls the global cursor) and calls
// window.__opmanChrome.set(shown). The overlay is a drag strip across the top;
// on Windows it also carries the minimize / maximize / close buttons.
(() => {
  if (window.top !== window || window.__opmanChrome) return;

  const platform = "__PLATFORM__";
  const height = platform === "windows" ? 32 : 28;
  const invoke = (cmd) =>
    window.__TAURI_INTERNALS__?.invoke(`plugin:window|${cmd}`, { label: "main" });

  let shown = false;
  let root = null;

  const css = `
    :host { all: initial; }
    .bar { position: fixed; inset: 0 0 auto 0; height: ${height}px; display: none;
      z-index: 2147483647; color: var(--fg, CanvasText); font: 10px "Segoe Fluent Icons", "Segoe MDL2 Assets"; }
    .bar.on { display: flex; }
    .drag { flex: 1; }
    button { all: unset; width: 46px; height: 100%; display: grid; place-items: center;
      color: inherit; cursor: default; transition: background-color 80ms; }
    button:hover { background: color-mix(in srgb, currentColor 10%, transparent); }
    button:active { background: color-mix(in srgb, currentColor 18%, transparent); }
    button.close:hover { background: #c42b1c; color: #fff; }
    button.close:active { background: #c42b1ccc; color: #fff; }`;

  const button = (cls, glyph, label, cmd) =>
    `<button class="${cls}" aria-label="${label}" data-cmd="${cmd}">${glyph}</button>`;
  const buttons =
    platform === "windows"
      ? button("min", "", "Minimize", "minimize") +
        button("max", "", "Maximize", "toggle_maximize") +
        button("close", "", "Close", "close")
      : "";

  const textColor = () => getComputedStyle(document.body || document.documentElement).color;

  const syncMaxGlyph = async () => {
    const max = root?.querySelector(".max");
    if (!max) return;
    const maximized = await invoke("is_maximized");
    max.textContent = maximized ? "" : "";
    max.setAttribute("aria-label", maximized ? "Restore" : "Maximize");
  };

  const mount = () => {
    if (root || !document.documentElement) return;
    const host = document.createElement("opman-titlebar");
    root = host.attachShadow({ mode: "closed" });
    root.innerHTML = `<style>${css}</style><div class="bar"><div class="drag"></div>${buttons}</div>`;
    const drag = root.querySelector(".drag");
    drag.addEventListener("mousedown", (event) => {
      if (event.button !== 0) return;
      if (event.detail === 2) return void invoke("toggle_maximize");
      invoke("start_dragging");
    });
    root.querySelectorAll("button").forEach((el) =>
      el.addEventListener("click", () => invoke(el.dataset.cmd)),
    );
    document.documentElement.append(host);
    window.addEventListener("resize", syncMaxGlyph, { passive: true });
    render();
  };

  const render = () => {
    const bar = root?.querySelector(".bar");
    if (!bar) return;
    bar.style.setProperty("--fg", textColor());
    bar.classList.toggle("on", shown);
    if (shown) syncMaxGlyph();
  };

  window.__opmanChrome = {
    set(next) {
      shown = next;
      mount();
      render();
    },
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", mount, { once: true });
  } else {
    mount();
  }
})();
