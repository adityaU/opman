import type { MissionStatus, PersonalMemoryItem, ProjectInfo } from "../api";
import type { PermissionRequest, QuestionRequest } from "../types";
import type { ActivityEvent, Mission } from "../api";

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
  onOpenMissionSource?: (missionId: string) => void;
}

export interface MissionHandoffCardProps {
  mission: Mission;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  activityEvents: ActivityEvent[];
  onOpenInbox?: () => void;
  onOpenMissionSource?: (missionId: string) => void;
}

export const STATUS_OPTIONS: MissionStatus[] = ["planned", "active", "blocked", "completed"];
