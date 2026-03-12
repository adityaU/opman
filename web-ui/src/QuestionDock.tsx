import React, { useState, useRef, useEffect, useCallback, useMemo } from "react";
import type { QuestionRequest } from "./types";
import { HelpCircle, Send, X } from "lucide-react";

interface Props {
  questions: QuestionRequest[];
  /** When set, questions from other sessions show a "subagent" badge */
  activeSessionId?: string | null;
  onReply: (requestId: string, answers: string[][]) => void;
  onDismiss: (requestId: string) => void;
}

export const QuestionDock = React.memo(function QuestionDock({ questions, activeSessionId, onReply, onDismiss }: Props) {
  const [activeTab, setActiveTab] = useState(0);

  // Clamp activeTab when questions list changes
  useEffect(() => {
    if (activeTab >= questions.length) {
      setActiveTab(Math.max(0, questions.length - 1));
    }
  }, [questions.length, activeTab]);

  if (questions.length === 0) return null;

  const showTabs = questions.length > 1;
  const activeQ = questions[Math.min(activeTab, questions.length - 1)];

  return (
    <div className="question-dock" role="region" aria-label="Questions">
      {showTabs && (
        <div className="dock-tabs dock-tabs--question">
          {questions.map((q, idx) => (
            <button
              key={q.id}
              className={`dock-tab dock-tab--question ${idx === activeTab ? "dock-tab--active" : ""}`}
              onClick={() => setActiveTab(idx)}
              aria-selected={idx === activeTab}
              role="tab"
            >
              <HelpCircle size={12} />
              <span className="dock-tab-label">
                {q.title || `Question ${idx + 1}`}
              </span>
              {!!activeSessionId && q.sessionID !== activeSessionId && (
                <span className="dock-tab-badge">sub</span>
              )}
            </button>
          ))}
        </div>
      )}
      {activeQ && (
        <QuestionCard
          key={activeQ.id}
          question={activeQ}
          isCrossSession={!!activeSessionId && activeQ.sessionID !== activeSessionId}
          onReply={onReply}
          onDismiss={onDismiss}
        />
      )}
    </div>
  );
});

function QuestionCard({
  question,
  isCrossSession,
  onReply,
  onDismiss,
}: {
  question: QuestionRequest;
  isCrossSession: boolean;
  onReply: (requestId: string, answers: string[][]) => void;
  onDismiss: (requestId: string) => void;
}) {
  const [answers, setAnswers] = useState<string[][]>(
    question.questions.map(() => [])
  );
  /** Custom free-text values for questions with custom=true */
  const [customTexts, setCustomTexts] = useState<string[]>(
    question.questions.map(() => "")
  );
  const cardRef = useRef<HTMLDivElement>(null);
  const firstInputRef = useRef<HTMLInputElement | null>(null);
  const firstButtonRef = useRef<HTMLButtonElement | null>(null);

  // Auto-focus the first interactive element when the card mounts
  useEffect(() => {
    const timer = setTimeout(() => {
      if (firstButtonRef.current) {
        firstButtonRef.current.focus();
      } else if (firstInputRef.current) {
        firstInputRef.current.focus();
      }
    }, 50);
    return () => clearTimeout(timer);
  }, [question.id]);

  const handleSubmit = useCallback(() => {
    // Merge selected options with custom text where applicable
    const finalAnswers = question.questions.map((q, idx) => {
      const selected = answers[idx] || [];
      const customText = customTexts[idx]?.trim();
      // If user typed custom text, use that (it takes priority when no option selected)
      if (customText && selected.length === 0) {
        return [customText];
      }
      // If user typed custom text AND selected options (multi-select), merge
      if (customText && selected.length > 0) {
        return [...selected, customText];
      }
      return selected;
    });
    onReply(question.id, finalAnswers);
  }, [question.id, answers, customTexts, onReply, question.questions]);

  const updateAnswer = (qIdx: number, value: string[]) => {
    setAnswers((prev) => {
      const next = [...prev];
      next[qIdx] = value;
      return next;
    });
  };

  const updateCustomText = (qIdx: number, value: string) => {
    setCustomTexts((prev) => {
      const next = [...prev];
      next[qIdx] = value;
      return next;
    });
  };

  /** True when every sub-question has at least one answer (selected option or custom text). */
  const hasAnswer = useMemo(() => {
    return question.questions.every((q, idx) => {
      const selected = answers[idx] || [];
      const customText = customTexts[idx]?.trim();
      if (q.type === "text") return (selected[0] || "").trim().length > 0;
      return selected.length > 0 || (customText ? customText.length > 0 : false);
    });
  }, [question.questions, answers, customTexts]);

  const handleDismiss = useCallback(() => {
    onDismiss(question.id);
  }, [question.id, onDismiss]);

  // Handle Enter to submit (only when answered) and Escape to dismiss
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        handleDismiss();
        return;
      }
      if (e.key === "Enter" && hasAnswer) {
        const target = e.target as HTMLElement;
        const isTextInput = target.tagName === "INPUT" && (target as HTMLInputElement).type === "text";
        if (!isTextInput || e.metaKey || e.ctrlKey) {
          e.preventDefault();
          handleSubmit();
        }
      }
    },
    [handleSubmit, handleDismiss, hasAnswer]
  );

  return (
    <div className="question-card" ref={cardRef} onKeyDown={handleKeyDown}>
      <div className="question-header">
        <HelpCircle size={16} className="question-icon" />
        <span className="question-title">{question.title || "Question"}</span>
        {isCrossSession && <span className="question-badge-subagent">subagent</span>}
        <span className="question-hint">Enter = submit &middot; Esc = dismiss</span>
        <button
          className="question-dismiss-btn"
          onClick={handleDismiss}
          aria-label="Dismiss question"
          title="Dismiss (Esc)"
        >
          <X size={14} />
        </button>
      </div>
      <div className="question-body">
        {question.questions.map((q, idx) => (
          <div key={idx} className="question-item">
            <label className="question-label">{q.text}</label>
            {q.type === "select" && q.options ? (
              <>
                <div className="question-options" role="listbox" aria-label={q.text}>
                  {q.options.map((opt, optIdx) => {
                    const selected = answers[idx]?.includes(opt);
                    const desc = q.optionDescriptions?.[optIdx];
                    return (
                      <button
                        key={opt}
                        ref={idx === 0 && optIdx === 0 ? firstButtonRef : undefined}
                        className={`question-option ${selected ? "selected" : ""}`}
                        role="option"
                        aria-selected={selected}
                        onClick={() => {
                          if (q.multiple) {
                            updateAnswer(
                              idx,
                              selected
                                ? answers[idx].filter((a) => a !== opt)
                                : [...(answers[idx] || []), opt]
                            );
                          } else {
                            // Clear custom text when selecting a predefined option
                            updateCustomText(idx, "");
                            updateAnswer(idx, [opt]);
                          }
                        }}
                      >
                        <span className="question-option-label">{opt}</span>
                        {desc && <span className="question-option-desc">{desc}</span>}
                      </button>
                    );
                  })}
                </div>
                {/* Custom free-text input when custom is enabled (default) */}
                {q.custom !== false && (
                  <input
                    ref={idx === 0 ? firstInputRef : undefined}
                    type="text"
                    className="question-text-input question-custom-input"
                    value={customTexts[idx] || ""}
                    onChange={(e) => {
                      updateCustomText(idx, e.target.value);
                      // Clear selected options when typing custom text
                      if (e.target.value) updateAnswer(idx, []);
                    }}
                    placeholder="Type your own answer..."
                    aria-label={`Custom answer for: ${q.text}`}
                  />
                )}
              </>
            ) : q.type === "confirm" ? (
              <div className="question-options" role="group" aria-label={q.text}>
                <button
                  ref={idx === 0 ? firstButtonRef : undefined}
                  className={`question-option ${answers[idx]?.[0] === "yes" ? "selected" : ""}`}
                  onClick={() => updateAnswer(idx, ["yes"])}
                  aria-pressed={answers[idx]?.[0] === "yes"}
                >
                  Yes
                </button>
                <button
                  className={`question-option ${answers[idx]?.[0] === "no" ? "selected" : ""}`}
                  onClick={() => updateAnswer(idx, ["no"])}
                  aria-pressed={answers[idx]?.[0] === "no"}
                >
                  No
                </button>
              </div>
            ) : (
              <input
                ref={idx === 0 ? firstInputRef : undefined}
                type="text"
                className="question-text-input"
                value={answers[idx]?.[0] || ""}
                onChange={(e) => updateAnswer(idx, [e.target.value])}
                placeholder="Type your answer..."
                aria-label={q.text}
              />
            )}
          </div>
        ))}
      </div>
      <div className="question-actions">
        <button
          className="question-submit-btn"
          onClick={handleSubmit}
          disabled={!hasAnswer}
          aria-label="Submit answers"
        >
          <Send size={14} />
          Submit
        </button>
      </div>
    </div>
  );
}
