import { createCmdlineState, type CmdlineModel } from "./cmdline";
import { createMessageState, type MessageModel } from "./messages";
import { createModeState, type ModeState } from "./modes";
import { createOptionState, type OptionState } from "./options";
import { createPopupmenuState, type PopupmenuModel } from "./popupmenu";
import { createTablineState, type TablineModel } from "./tabline";

export interface NvimState {
  readonly modes: ModeState;
  readonly cmdline: CmdlineModel;
  readonly messages: MessageModel;
  readonly popupmenu: PopupmenuModel;
  readonly tabline: TablineModel;
  readonly options: OptionState;
}

export function createNvimState(): NvimState {
  return {
    modes: createModeState(),
    cmdline: createCmdlineState(),
    messages: createMessageState(),
    popupmenu: createPopupmenuState(),
    tabline: createTablineState(),
    options: createOptionState(),
  };
}

export class NvimStore {
  readonly state: NvimState;

  constructor(state = createNvimState()) {
    this.state = state;
  }
}

export function createStore(): NvimStore {
  return new NvimStore();
}
