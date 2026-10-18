import React, { useEffect, useRef, useState } from "react";
import { Plus, X } from "lucide-react";
import { browserClose } from "../api/browser";
import { storageKey } from "../api/base";
import { BrowserTabPanel, type BrowserPanelProps } from "./BrowserPanel";
import "../styles/browser-tabs.css";

interface Tab { id: string; url: string | null }
interface Tabs { tabs: Tab[]; active: string }

function loadTabs(id: string, url: string | null): Tabs {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(storageKey(`opman.browser.tabs:${id}`)) ?? "null");
    if (value && typeof value === "object" && "tabs" in value && "active" in value) {
      const saved = value as Tabs;
      if (Array.isArray(saved.tabs) && saved.tabs.length > 0 && saved.tabs.every(tab =>
        typeof tab.id === "string" && (tab.id === id || tab.id.startsWith(`${id}:tab:`)) &&
        (tab.url === null || typeof tab.url === "string")) &&
        saved.tabs.some(tab => tab.id === saved.active)) return saved;
    }
  } catch { /* Storage may be blocked. Keep this session in memory. */ }
  return { tabs: [{ id, url }], active: id };
}

function label(url: string | null): string {
  if (!url || url === "about:blank") return "New tab";
  try { return new URL(url).hostname || url; } catch { return url; }
}

export function BrowserPanel(props: BrowserPanelProps) {
  return <BrowserTabsState key={props.paneId} {...props} />;
}

function BrowserTabsState(props: BrowserPanelProps) {
  const { paneId, url, reveal, onUrlChanged } = props;
  const [state, setState] = useState(() => loadTabs(paneId, url));
  const lastReveal = useRef(-1);
  const tabButtons = useRef(new Map<string, HTMLButtonElement>());
  const focusRequested = useRef(false);

  useEffect(() => {
    if (!focusRequested.current) return;
    focusRequested.current = false;
    tabButtons.current.get(state.active)?.focus();
  }, [state.active]);

  useEffect(() => {
    const navigate = (event: Event) => {
      const detail = (event as CustomEvent<{ projectPath: string; url: string }>).detail;
      if (detail.projectPath !== props.project) return;
      setState(previous => ({
        tabs: previous.tabs.some(tab => tab.id === paneId)
          ? previous.tabs.map(tab => tab.id === paneId ? { ...tab, url: detail.url } : tab)
          : [...previous.tabs, { id: paneId, url: detail.url }],
        active: paneId,
      }));
    };
    window.addEventListener("opman:browser-agent-page", navigate);
    return () => window.removeEventListener("opman:browser-agent-page", navigate);
  }, [paneId, props.project]);

  useEffect(() => {
    try { localStorage.setItem(storageKey(`opman.browser.tabs:${paneId}`), JSON.stringify(state)); } catch { /* Optional. */ }
  }, [paneId, state]);

  useEffect(() => {
    if (lastReveal.current === reveal) return;
    lastReveal.current = reveal;
    if (!url) return;
    setState(previous => {
      const existing = previous.tabs.find(tab => tab.url === url);
      if (existing) return { ...previous, active: existing.id };
      const id = `${paneId}:tab:${crypto.randomUUID()}`;
      return { tabs: [...previous.tabs, { id, url }], active: id };
    });
  }, [paneId, reveal, url]);

  const active = state.tabs.find(tab => tab.id === state.active) ?? state.tabs[0];
  const addTab = () => {
    const id = `${paneId}:tab:${crypto.randomUUID()}`;
    focusRequested.current = true;
    setState(previous => ({ tabs: [...previous.tabs, { id, url: null }], active: id }));
  };
  const closeTab = (id: string) => {
    focusRequested.current = state.active === id;
    void browserClose(id).catch(() => {});
    setState(previous => {
      const index = previous.tabs.findIndex(tab => tab.id === id);
      const tabs = previous.tabs.filter(tab => tab.id !== id);
      if (tabs.length === 0) {
        const nextId = `${paneId}:tab:${crypto.randomUUID()}`;
        return { tabs: [{ id: nextId, url: null }], active: nextId };
      }
      return { tabs, active: previous.active === id ? tabs[Math.max(0, index - 1)].id : previous.active };
    });
  };

  return <div className="bwp-tabs-panel">
    <div className="bwp-tabstrip" role="tablist" aria-label="Browser tabs" onKeyDown={event => {
      if (!(event.target instanceof HTMLElement) || event.target.getAttribute("role") !== "tab") return;
      const index = state.tabs.findIndex(tab => tab.id === state.active);
      const target = event.key === "Home" ? 0 : event.key === "End" ? state.tabs.length - 1
        : event.key === "ArrowRight" ? (index + 1) % state.tabs.length
        : event.key === "ArrowLeft" ? (index + state.tabs.length - 1) % state.tabs.length : null;
      if (target === null) return;
      event.preventDefault();
      focusRequested.current = true;
      setState(previous => ({ ...previous, active: previous.tabs[target].id }));
      if (target === index) {
        focusRequested.current = false;
        tabButtons.current.get(state.active)?.focus();
      }
    }}>
      {state.tabs.map(tab => <div className={`bwp-tab${tab.id === active.id ? " is-active" : ""}`} key={tab.id}>
        <button type="button" role="tab" aria-selected={tab.id === active.id}
          id={`browser-tab-${tab.id}`} tabIndex={tab.id === active.id ? 0 : -1}
          ref={button => { if (button) tabButtons.current.set(tab.id, button); else tabButtons.current.delete(tab.id); }}
          aria-controls={`browser-${tab.id}`} title={tab.url ?? "New tab"}
          onClick={() => setState(previous => ({ ...previous, active: tab.id }))}>{label(tab.url)}</button>
        <button type="button" className="bwp-tab-close" aria-label={`Close ${label(tab.url)}`}
          onClick={() => closeTab(tab.id)}><X size={12} /></button>
      </div>)}
      <button type="button" className="bwp-nav" aria-label="New browser tab" onClick={addTab}><Plus size={14} /></button>
    </div>
    <div className="bwp-tabcontent" role="tabpanel" id={`browser-${active.id}`} aria-labelledby={`browser-tab-${active.id}`}>
      <BrowserTabPanel {...props} key={active.id} paneId={active.id} url={active.url} reveal={0}
        onUrlChanged={next => {
          setState(previous => ({ ...previous, tabs: previous.tabs.map(tab => tab.id === active.id ? { ...tab, url: next } : tab) }));
          onUrlChanged(next);
        }} />
    </div>
  </div>;
}
