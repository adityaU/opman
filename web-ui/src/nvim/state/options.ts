export interface OptionState {
  guifont: string;
  linespace: number;
  title: string;
  mouseEnabled: boolean;
  bellCount: number;
  busy: boolean;
}

export function createOptionState(): OptionState {
  return {
    guifont: "", linespace: 0, title: "", mouseEnabled: true, bellCount: 0, busy: false,
  };
}

export function setOption(state: OptionState, name: string, value: boolean | number | string): void {
  if (name === "guifont") state.guifont = String(value);
  else if (name === "linespace") state.linespace = typeof value === "number" ? value : Number(value) || 0;
}

export function setTitle(state: OptionState, title: string): void {
  state.title = title;
}

export function setMouse(state: OptionState, enabled: boolean): void {
  state.mouseEnabled = enabled;
}

export function ringBell(state: OptionState): void {
  state.bellCount += 1;
}

export function setBusy(state: OptionState, busy: boolean): void {
  state.busy = busy;
}
