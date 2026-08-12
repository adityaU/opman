import { sendNvimEditClientMsg } from "../../api/nvim-ui";
import { parseControl, type ClientMsg, type ControlMsg } from "./wire";

export interface SocketCallbacks {
  readonly onOpen: () => void;
  readonly onControl: (message: ControlMsg) => void;
  readonly onInvalid: () => void;
  readonly onError: () => void;
  readonly onClose: () => void;
}

/** The edit channel's WebSocket and its retry timer, with no editor state. */
export class NvimSocket {
  private socket: WebSocket | null = null;
  private timer: number | null = null;

  /** Returns false when the socket could not even be constructed. */
  open(url: string, callbacks: SocketCallbacks): boolean {
    let socket: WebSocket;
    try {
      socket = new WebSocket(url);
    } catch {
      return false;
    }
    this.socket = socket;
    socket.onopen = () => callbacks.onOpen();
    socket.onmessage = (event: MessageEvent): void => {
      if (typeof event.data !== "string") return;
      try {
        const message = parseControl(JSON.parse(event.data) as unknown);
        if (message) callbacks.onControl(message);
      } catch {
        callbacks.onInvalid();
      }
    };
    socket.onerror = () => callbacks.onError();
    socket.onclose = () => {
      if (this.socket !== socket) return;
      this.socket = null;
      callbacks.onClose();
    };
    return true;
  }

  get isConnecting(): boolean {
    return this.socket?.readyState === WebSocket.CONNECTING;
  }

  get isOpen(): boolean {
    return this.socket?.readyState === WebSocket.OPEN;
  }

  get isIdle(): boolean {
    return this.socket === null && this.timer === null;
  }

  send(message: ClientMsg): boolean {
    return this.socket !== null && sendNvimEditClientMsg(this.socket, message);
  }

  retry(delayMs: number, action: () => void): void {
    if (this.timer !== null) return;
    this.timer = window.setTimeout(() => {
      this.timer = null;
      action();
    }, delayMs);
  }

  /** Drop the socket without letting its close event reopen the connection. */
  close(): void {
    if (this.timer !== null) window.clearTimeout(this.timer);
    this.timer = null;
    const socket = this.socket;
    this.socket = null;
    socket?.close();
  }
}
