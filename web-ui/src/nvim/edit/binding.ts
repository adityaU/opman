import { Annotation, EditorSelection, type Extension } from "@codemirror/state";
import { EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { buildNvimUiUrl } from "../../api/nvim-ui";
import { NvimSocket } from "./socket";
import { InputQueue } from "./inputQueue";
import { routeNvimKey } from "./keyRouting";
import { attachedText, editFromUpdate, lineChangeToSpec, type EditRecord } from "./sync";
import {
  bindingDecorations, bindingSnapshotEffect, bindingStateField,
  initialBindingSnapshot, type BindingSnapshot, type IdleReason,
} from "./decorations";
import { nvimStatusPanelExtension } from "./panel";
import { nvimOverlayPanelExtension } from "./overlayPanel";
import { nvimTablinePanelExtension } from "./tablinePanel";
import { emptyOverlays, reduceOverlays } from "./overlays";
import { offsetToPosition, positionToOffset } from "./columns";
import type { ClientMsg, ControlMsg, TextPosition } from "./wire";
import { messageToSnapshot } from "./snapshot";
import { editId, isInsertMode, type BindingOptions } from "./binding-types";
import { hideCompletionFallback, showCompletionFallback } from "./completionFallback";
export const nvimTransaction = Annotation.define<"remote_document" | "remote_state">();
export class NvimEditBinding {
  readonly extension: Extension;
  private options: BindingOptions = { enabled: false, path: null, sessionId: null, idleReason: "disabled" };
  private readonly socket = new NvimSocket(); private view: EditorView | null = null;
  private keydownHandler: ((event: KeyboardEvent) => void) | null = null;
  private disposed = false; private sequence = 0; private changedtick = 0;
  private terminalFailure = false;
  private snapshot: BindingSnapshot = initialBindingSnapshot;
  private editInFlight: EditRecord | null = null;
  private readonly pendingEdits: EditRecord[] = [];
  private readonly inputs = new InputQueue();
  private normalModePending = false;
  private stateReady = false;
  private hasSeenState = false;
  private pendingCaret: { readonly tick: number; readonly cursor: TextPosition } | null = null;
  private pendingCursor: TextPosition | null = null; private remoteTick = -1;
  constructor() {
    this.extension = [bindingStateField, bindingDecorations, nvimTablinePanelExtension,
      nvimOverlayPanelExtension, nvimStatusPanelExtension,
      ViewPlugin.define((view) => { this.attachView(view); return { destroy: () => this.detachView(view) }; }),
      EditorView.updateListener.of((update) => this.onUpdate(update)),
      EditorView.editorAttributes.of({ "aria-label": "Code editor with Neovim keybindings", "aria-keyshortcuts": "Control+Shift+Escape" })];
  }
  setOptions(options: BindingOptions): void {
    this.disposed = false;
    const changed = options.enabled !== this.options.enabled || options.path !== this.options.path
      || options.sessionId !== this.options.sessionId || options.idleReason !== this.options.idleReason;
    this.options = options;
    if (!changed) return;
    this.stopConnection();
    this.terminalFailure = false;
    if (!this.idleReason()) this.connect();
  }
  attachView(view: EditorView): void {
    this.view = view;
    const handler = (event: KeyboardEvent): void => {
      if (!(event.target instanceof Node) || !view.dom.contains(event.target)) return;
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return;
      if (this.routeKey(view, event)) event.stopPropagation();
    };
    this.keydownHandler = handler;
    document.addEventListener("keydown", handler, true);
    queueMicrotask(() => {
      if (this.view === view) this.syncView(view);
    });
  }
  private syncView(_view: EditorView): void {
    if (this.options.enabled && this.options.path && this.options.sessionId) {
      if (this.socket.isIdle) this.connect();
      else if (this.socket.isConnecting) {
        this.applySnapshot({ ...this.snapshot, connection: { status: "connecting" }, message: null });
      } else this.dispatchSnapshot(this.snapshot, false);
      return;
    }
    this.dispatchSnapshot(this.snapshot, false);
  }
  detachView(view: EditorView): void {
    if (this.view !== view) return;
    if (this.keydownHandler) document.removeEventListener("keydown", this.keydownHandler, true);
    this.keydownHandler = null;
    this.view = null;
  }
  dispose(): void {
    this.disposed = true; this.stopConnection();
    this.view = null;
  }
  private connect(): void {
    const { sessionId, path } = this.options;
    if (this.disposed || this.idleReason() || !path || !sessionId) return;
    this.applySnapshot({ ...this.snapshot, connection: { status: "connecting" }, message: null });
    this.stateReady = false;
    const opened = this.socket.open(buildNvimUiUrl({ sessionId }), {
      onOpen: () => {
        this.send({ type: "attach", path });
        this.applySnapshot({ ...this.snapshot, connection: { status: "connecting" }, message: null });
      },
      onControl: (message) => this.handleControl(message),
      onInvalid: () => this.fail("Invalid Neovim control message"),
      onError: () => this.fail("Neovim connection failed"),
      onClose: () => {
        if (this.disposed || !this.options.enabled || this.terminalFailure) return;
        this.scheduleReconnect("Neovim disconnected; retrying");
      },
    });
    if (!opened) this.scheduleReconnect("Unable to connect to Neovim");
  }
  /** Report why no connection is wanted, or null when one is. */
  private idleReason(): IdleReason | null {
    const { enabled, path, sessionId, idleReason } = this.options;
    const reason: IdleReason | null = !enabled ? idleReason : !path ? "no-file" : !sessionId ? "no-session" : null;
    if (reason) this.applySnapshot({ ...this.snapshot, connection: { status: "idle", reason }, message: null });
    return reason;
  }
  private fail(reason: string): void {
    this.applySnapshot({ ...this.snapshot, connection: { status: "failed", reason }, message: null });
  }
  private scheduleReconnect(message: string): void {
    if (this.terminalFailure) return;
    this.fail(message);
    if (this.disposed) return;
    this.socket.retry(1500, () => this.connect());
  }
  private stopConnection(): void {
    this.socket.close();
    this.editInFlight = null;
    this.pendingEdits.length = 0;
    this.inputs.clear();
    this.normalModePending = false; this.stateReady = false; this.pendingCaret = null; this.pendingCursor = null;
  }
  private send(message: ClientMsg): boolean { return this.socket.send(message); }
  private routeKey(view: EditorView, event: KeyboardEvent): boolean {
    const mode = this.normalModePending ? "normal" : this.snapshot.modeShort;
    const routed = routeNvimKey(
      event,
      mode,
      (keys) => this.enqueueInput(keys),
      () => { view.contentDOM.blur(); },
    );
    if (routed && mode === "insert" && event.ctrlKey && event.key === "n") showCompletionFallback(view);
    if (routed && event.key === "Escape") hideCompletionFallback(view);
    return routed;
  }
  private enqueueInput(keys: string): void {
    if (keys === "<Esc>") this.normalModePending = true;
    this.inputs.push(keys);
    this.pendingCursor = null;
    this.pumpInput();
  }
  private pumpInput(): void {
    const ready = this.editInFlight === null && this.pendingEdits.length === 0
      && this.stateReady && this.socket.isOpen;
    if (!this.inputs.pump(ready, (keys) => this.send({ type: "input", keys }))) this.fail("Neovim is not connected");
  }
  private onUpdate(update: ViewUpdate): void {
    if (update.view === this.view && !update.docChanged && update.selectionSet
      && !update.transactions.some((transaction) => transaction.annotation(nvimTransaction) !== undefined)) {
      const position = offsetToPosition(update.state.doc, update.state.selection.main.head);
      this.pendingCursor = position;
      this.send({ type: "cursor", position });
    }
    if (update.view !== this.view || !update.docChanged
      || update.transactions.some((transaction) => transaction.annotation(nvimTransaction) !== undefined)
      || !this.options.enabled || !this.options.path) return;
    const change = editFromUpdate(update);
    if (!change) return;
    const id = editId(++this.sequence);
    this.pendingEdits.push({ editId: id, payload: change, baseTick: 0 });
    this.pumpEdit();
  }
  private overlay(message: Parameters<typeof reduceOverlays>[1]): void {
    const opening = message.type === "cmdline" && message.visible;
    this.applySnapshot({ ...this.snapshot, ...(opening ? { message: null } : {}),
      overlays: reduceOverlays(this.snapshot.overlays, message) });
  }
  private handleControl(message: ControlMsg): void {
    switch (message.type) {
      case "cmdline": case "search": case "layout": this.overlay(message); return;
      case "action": this.options.onAction?.(message.name); return;
      case "attached": this.handleAttached(message); return;
      case "buffer_changed": this.handleBufferChanged(message); return;
      case "state": {
        const next = messageToSnapshot(message);
        this.hasSeenState = true;
        if (this.pendingCursor) {
          if (this.pendingCursor.line === message.cursor.line && this.pendingCursor.column === message.cursor.column) this.pendingCursor = null;
          else this.send({ type: "cursor", position: this.pendingCursor });
        }
        const cursorPending = this.pendingCursor !== null;
        const entering = isInsertMode(next.modeShort) && !isInsertMode(this.snapshot.modeShort);
        const settled = message.changedtick === this.changedtick;
        this.pendingCaret = entering && !settled ? { tick: message.changedtick, cursor: message.cursor } : null;
        this.changedtick = message.changedtick; this.stateReady = true;
        if (message.mode_short !== "insert") this.normalModePending = false;
        // The local caret leads only for edits this editor originated. After a
        // change Neovim made, Neovim's cursor is the truth in every mode.
        const follow = !cursorPending && (!isInsertMode(next.modeShort) || message.changedtick === this.remoteTick || (entering && settled));
        this.applySnapshot({ ...this.snapshot, ...next }, follow);
        this.pumpInput();
        return;
      }
      case "input_ack": this.inputs.acknowledge(); this.pumpInput(); return;
      case "resync_required": this.resync(message.reason); return;
      case "buffer_detached":
        if (message.reason !== "Neovim deleted the buffer") { this.resync(message.reason); return; }
        this.stopConnection(); this.applySnapshot({ ...this.snapshot, connection: { status: "idle", reason: "no-file" }, message: null });
        this.options.onBufferDetached?.(); return;
      case "message":
        this.applySnapshot({ ...this.snapshot, changedtick: message.changedtick, message: message.text,
          overlays: reduceOverlays(this.snapshot.overlays, message) });
        return;
      case "command_output": this.applySnapshot({ ...this.snapshot, changedtick: message.changedtick, message: message.output }); return;
      case "error": this.applySnapshot({ ...this.snapshot, connection: { status: "failed", reason: message.message }, message: null }); return;
      case "exited":
        this.terminalFailure = true;
        this.socket.close();
        this.fail("Neovim exited");
        return;
      case "superseded": this.applySnapshot({ ...this.snapshot, connection: { status: "failed", reason: "Neovim view was taken over" }, message: null }); return;
      case "too_slow": this.applySnapshot({ ...this.snapshot, connection: { status: "failed", reason: "Neovim is sending updates too quickly" }, message: null }); return;
      case "ready": return;
    }
  }
  private handleAttached(message: Extract<ControlMsg, { type: "attached" }>): void {
    this.changedtick = message.changedtick; this.remoteTick = message.changedtick;
    this.stateReady = this.hasSeenState;
    this.editInFlight = null;
    this.pendingEdits.length = 0;
    const snapshot = { ...this.snapshot, changedtick: message.changedtick,
      connection: { status: "attached" } as const, message: null, overlays: emptyOverlays };
    const view = this.view;
    const text = attachedText(message.lines);
    if (!view || view.state.doc.toString() === text) { this.applySnapshot(snapshot, false); this.pumpInput(); return; }
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text },
      effects: bindingSnapshotEffect.of(snapshot), annotations: nvimTransaction.of("remote_document") });
    this.snapshot = snapshot;
    this.pumpInput();
  }
  private handleBufferChanged(message: Extract<ControlMsg, { type: "buffer_changed" }>): void {
    if (message.origin && this.editInFlight?.editId === message.origin) {
      this.editInFlight = null;
      this.changedtick = message.changedtick;
      this.snapshot = { ...this.snapshot, changedtick: message.changedtick };
      this.pumpEdit();
      this.pumpInput();
      return;
    }
    if (this.editInFlight || this.pendingEdits.length > 0) {
      this.resync("another change arrived while an edit was in flight");
      return;
    }
    const view = this.view;
    const spec = view ? lineChangeToSpec(view.state.doc, message) : null;
    if (!view || !spec) {
      this.resync("Neovim sent an invalid line range");
      return;
    }
    this.changedtick = message.changedtick; this.remoteTick = message.changedtick;
    const caret = this.pendingCaret?.tick === message.changedtick ? this.pendingCaret.cursor : null;
    // Text Neovim inserted at the caret belongs before it, not after: map with
    // forward association so local typing continues past what just arrived.
    const changes = view.state.changes(spec);
    const head = isInsertMode(this.snapshot.modeShort) ? changes.mapPos(view.state.selection.main.head, 1) : null;
    view.dispatch({ changes, ...(head === null ? {} : { selection: EditorSelection.cursor(head) }),
      annotations: nvimTransaction.of("remote_document") });
    if (caret) {
      this.pendingCaret = null;
      view.dispatch({ selection: EditorSelection.cursor(positionToOffset(view.state.doc, caret)),
        annotations: nvimTransaction.of("remote_state") });
    }
    this.snapshot = { ...this.snapshot, changedtick: message.changedtick };
  }
  private resync(reason: string): void {
    this.editInFlight = null;
    this.pendingEdits.length = 0;
    this.applySnapshot({ ...this.snapshot, connection: { status: "attached" }, message: `Resyncing buffer: ${reason}` });
    if (this.options.path) this.send({ type: "attach", path: this.options.path });
  }
  private pumpEdit(): void {
    if (this.editInFlight || this.pendingEdits.length === 0) return;
    if (!this.socket.isOpen) return;
    const next = this.pendingEdits.shift();
    if (!next) return;
    const record = { ...next, baseTick: this.changedtick };
    this.editInFlight = record;
    if (!this.send({ ...record.payload, type: "edit", changedtick: record.baseTick, edit_id: record.editId })) {
      this.editInFlight = null;
      this.applySnapshot({ ...this.snapshot, connection: { status: "failed", reason: "Neovim is not connected" }, message: null });
    }
  }
  private applySnapshot(snapshot: BindingSnapshot, moveSelection = false): void {
    this.snapshot = snapshot; this.dispatchSnapshot(snapshot, moveSelection);
  }
  private dispatchSnapshot(snapshot: BindingSnapshot, moveSelection: boolean): void {
    const view = this.view;
    if (!view) return;
    const selection = moveSelection && snapshot.cursor ? EditorSelection.cursor(positionToOffset(view.state.doc, snapshot.cursor)) : undefined;
    view.dispatch({ ...(selection ? { selection } : {}), effects: bindingSnapshotEffect.of(snapshot), annotations: nvimTransaction.of("remote_state") });
  }
}
