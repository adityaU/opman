import React, { useState, useRef, useEffect, useCallback, useMemo } from "react";
import type { QuestionRequest, QuestionItem } from "./types";
import { HelpCircle, Send } from "lucide-react";
import { DockCard, DockTabs, type DockTab } from "./dock/DockCard";
import { useClampedTab } from "./dock/useClampedTab";
import { QuestionField } from "./dock/QuestionField";

interface Props {
  questions: QuestionRequest[];
  /** When set, questions from other sessions show a "subagent" badge */
  activeSessionId?: string | null;
  onReply: (requestId: string, answers: string[][]) => void;
  onDismiss: (requestId: string) => void;
  /** Navigate to a session by its ID */
  onGoToSession?: (sessionId: string) => void;
}

export const QuestionDock = React.memo(function QuestionDock({
  questions,
  activeSessionId,
  onReply,
  onDismiss,
  onGoToSession,
}: Props) {
  const [activeTab, setActiveTab] = useClampedTab(questions.length);

  const tabs = useMemo<DockTab[]>(
    () =>
      questions.map((request, index) => ({
        id: request.id,
        label: request.title || `Question ${index + 1}`,
        icon: <HelpCircle size={12} />,
        badge: !!activeSessionId && request.sessionID !== activeSessionId ? "sub" : undefined,
      })),
    [questions, activeSessionId],
  );

  if (questions.length === 0) return null;
  const active = questions[activeTab];
  if (!active) return null;

  return (
    <div className="dock-panel dock-panel--question" role="region" aria-label="Questions">
      <DockTabs tabs={tabs} active={activeTab} onSelect={setActiveTab} kind="question" label="Pending questions" />
      <QuestionCard
        key={active.id}
        request={active}
        isCrossSession={!!activeSessionId && active.sessionID !== activeSessionId}
        onReply={onReply}
        onDismiss={onDismiss}
        onGoToSession={onGoToSession}
      />
    </div>
  );
});

/** True when this question has something to send: a chosen option or typed text. */
function isAnswered(question: QuestionItem, answer: readonly string[], custom: string): boolean {
  if (question.type === "text") return (answer[0] || "").trim().length > 0;
  return answer.length > 0 || custom.trim().length > 0;
}

function QuestionCard({
  request,
  isCrossSession,
  onReply,
  onDismiss,
  onGoToSession,
}: {
  request: QuestionRequest;
  isCrossSession: boolean;
  onReply: (requestId: string, answers: string[][]) => void;
  onDismiss: (requestId: string) => void;
  onGoToSession?: (sessionId: string) => void;
}) {
  const items = request.questions;
  const [answers, setAnswers] = useState<string[][]>(() => items.map(() => []));
  /** Free-text values for questions that also accept a custom answer. */
  const [customTexts, setCustomTexts] = useState<string[]>(() => items.map(() => ""));
  const [current, setCurrent] = useClampedTab(items.length);
  const cardRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const buttonRef = useRef<HTMLButtonElement | null>(null);

  const handleDismiss = useCallback(() => onDismiss(request.id), [request.id, onDismiss]);

  // Focus the first control of the visible question so the keyboard works right away.
  useEffect(() => {
    const timer = setTimeout(() => {
      const target = buttonRef.current || inputRef.current || cardRef.current;
      target?.focus();
    }, 50);
    return () => clearTimeout(timer);
  }, [request.id, current]);

  // Escape has to work even when focus sits outside the card.
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      handleDismiss();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleDismiss]);

  const answeredFlags = useMemo(
    () => items.map((question, index) => isAnswered(question, answers[index] || [], customTexts[index] || "")),
    [items, answers, customTexts],
  );
  const answeredCount = answeredFlags.filter(Boolean).length;
  const hasAnswer = answeredCount === items.length;

  const handleSubmit = useCallback(() => {
    // A typed answer wins on single-select; on multi-select it joins the chosen options.
    const finalAnswers = items.map((_, index) => {
      const selected = answers[index] || [];
      const custom = customTexts[index]?.trim();
      if (!custom) return selected;
      return selected.length === 0 ? [custom] : [...selected, custom];
    });
    onReply(request.id, finalAnswers);
  }, [request.id, items, answers, customTexts, onReply]);

  const updateAnswer = useCallback((index: number, value: string[]) => {
    setAnswers((previous) => previous.map((entry, position) => (position === index ? value : entry)));
  }, []);

  const updateCustomText = useCallback((index: number, value: string) => {
    setCustomTexts((previous) => previous.map((entry, position) => (position === index ? value : entry)));
  }, []);

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        handleDismiss();
        return;
      }
      if (event.key !== "Enter" || !hasAnswer) return;
      event.preventDefault();
      handleSubmit();
    },
    [handleSubmit, handleDismiss, hasAnswer],
  );

  const tabs = useMemo<DockTab[]>(
    () =>
      items.map((question, index) => ({
        id: `${request.id}-${index}`,
        label: question.header || `Q${index + 1}`,
        done: answeredFlags[index],
      })),
    [items, request.id, answeredFlags],
  );

  const question = items[current];
  const multi = items.length > 1;
  const footer = (
    <>
      {multi && (
        <span className="dock-progress" aria-live="polite">
          {answeredCount} of {items.length} answered
        </span>
      )}
      <button
        type="button"
        className="dock-btn dock-btn--submit"
        onClick={handleSubmit}
        disabled={!hasAnswer}
        aria-label="Submit answers"
      >
        <Send size={14} />
        Submit
      </button>
    </>
  );

  return (
    <DockCard
      kind="question"
      icon={<HelpCircle size={16} />}
      title={request.title || "Question"}
      subtitle={multi ? `${items.length} questions` : undefined}
      isCrossSession={isCrossSession}
      sessionId={request.sessionID}
      onGoToSession={onGoToSession}
      hint="Enter = submit · Esc = dismiss"
      onDismiss={handleDismiss}
      dismissLabel="Dismiss question"
      tabs={tabs}
      activeTab={current}
      onSelectTab={setCurrent}
      footer={footer}
      cardRef={cardRef}
      onKeyDown={handleKeyDown}
    >
      {question && (
        <div className="dock-field">
          <label className="dock-field-label">{question.text}</label>
          <QuestionField
            question={question}
            answer={answers[current] || []}
            customText={customTexts[current] || ""}
            onAnswer={(value) => updateAnswer(current, value)}
            onCustomText={(value) => updateCustomText(current, value)}
            buttonRef={buttonRef}
            inputRef={inputRef}
          />
        </div>
      )}
    </DockCard>
  );
}
