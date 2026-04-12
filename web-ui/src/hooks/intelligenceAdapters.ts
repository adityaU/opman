/**
 * Adapters to convert transient frontend types (PermissionRequest,
 * QuestionRequest, AssistantSignal) into the shapes expected by the
 * backend intelligence API endpoints.
 */

import type { PermissionInput, QuestionInput, SignalInput } from "../api/intelligence";
import type { PermissionRequest, QuestionRequest } from "../types";
import type { AssistantSignal } from "./useAssistantState";

export function toPermissionInputs(permissions: PermissionRequest[]): PermissionInput[] {
  return permissions.map((p) => ({
    id: p.id,
    sessionID: p.sessionID,
    toolName: p.toolName,
    description: p.description,
    time: p.time,
  }));
}

export function toQuestionInputs(questions: QuestionRequest[]): QuestionInput[] {
  return questions.map((q) => ({
    id: q.id,
    sessionID: q.sessionID,
    title: q.title,
    time: q.time,
  }));
}

export function toSignalInputs(signals: AssistantSignal[]): SignalInput[] {
  return signals.map((s) => ({
    id: s.id,
    kind: s.kind,
    title: s.title,
    body: s.body,
    created_at: s.createdAt,
    session_id: s.sessionId,
  }));
}
