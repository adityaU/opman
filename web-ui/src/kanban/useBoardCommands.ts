import { useCommands } from "../keybindings/useCommand";
import type { Task } from "../api/kanban";
import type { BoardSelection } from "./useBoardSelection";

/**
 * Registers the board's commands.
 *
 * Card-level actions read the keyboard selection, so `x` launches whatever
 * `j`/`k`/`h`/`l` last landed on. Moving a card across lanes goes through the
 * board's own transition rules, so the keyboard cannot make a move the pointer
 * would have refused.
 */

export interface BoardCommandDeps {
  readonly selection: BoardSelection;
  readonly newTask: () => void;
  readonly configureLanes: () => void;
  readonly refresh: () => void;
  readonly openDetail: (task: Task) => void;
  readonly editTask: (task: Task) => void;
  readonly launchTask: (task: Task) => void;
  readonly openSession: (sessionId: string) => void;
  readonly archiveTask: (taskId: string, archived: boolean) => void;
  readonly moveTaskLane: (task: Task, delta: number) => void;
}

export function useBoardCommands(deps: BoardCommandDeps): void {
  const onSelection = (run: (task: Task) => void) => () => {
    const task = deps.selection.selectedTask;
    if (task) run(task);
  };

  useCommands({
    "board.newTask": deps.newTask,
    "board.configureLanes": deps.configureLanes,
    "board.refresh": deps.refresh,
    "board.moveDown": deps.selection.moveDown,
    "board.moveUp": deps.selection.moveUp,
    "board.moveLeft": deps.selection.moveLeft,
    "board.moveRight": deps.selection.moveRight,
    "board.openTask": onSelection(deps.openDetail),
    "board.editTask": onSelection(deps.editTask),
    "board.launch": onSelection(deps.launchTask),
    "board.openTaskSession": onSelection((task) => {
      if (task.session_id) deps.openSession(task.session_id);
    }),
    "board.archiveTask": onSelection((task) => deps.archiveTask(task.id, true)),
    "board.moveTaskLeft": onSelection((task) => deps.moveTaskLane(task, -1)),
    "board.moveTaskRight": onSelection((task) => deps.moveTaskLane(task, 1)),
  });
}
