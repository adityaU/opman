import type { PersonalMemoryItem, ProjectInfo, MissionState } from "../api";
import type { PermissionRequest, QuestionRequest } from "../types";
import type { ActivityEvent } from "../api";

export interface MissionsModalProps {
  onClose: () => void;
  projects: ProjectInfo[];
  activeProjectIndex: number;
  activeSessionId: string | null;
  permissions?: PermissionRequest[];
  questions?: QuestionRequest[];
  activityEvents?: ActivityEvent[];
  onOpenInbox?: () => void;
  activeMemoryItems?: PersonalMemoryItem[];
}

export const STATE_ORDER: MissionState[] = [
  "executing",
  "evaluating",
  "pending",
  "paused",
  "completed",
  "failed",
  "cancelled",
];
