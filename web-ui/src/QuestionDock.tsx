import React, { useState, useRef, useEffect, useCallback } from "react";
import type { QuestionRequest } from "./types";
import { HelpCircle, Send } from "lucide-react";

interface Props {
  questions: QuestionRequest[];
  /** When set, questions from other sessions show a "subagent" badge */
  activeSessionId?: string | null;
  onReply: (requestId: string, answers: string[][]) => void;
}

export const QuestionDock = React.memo(function QuestionDock({ questions, activeSessionId, onReply }: Props) {
  return (
    <div className="question-dock" role="region" aria-label="Questions">
      {questions.map((q) => (
        <QuestionCard
          key={q.id}
          question={q}
          isCrossSession={!!activeSessionId && q.sessionID !== activeSessionId}
          onReply={onReply}
        />
      ))}
    </div>
  );
});

function QuestionCard({
  question,
  isCrossSession,
  onReply,
}: {
  question: QuestionRequest;
  isCrossSession: boolean;
  onReply: (requestId: string, answers: string[][]) => void;
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

  // Handle Enter to submit from anywhere in the card
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      // Enter submits (Cmd/Ctrl+Enter for text inputs to allow normal Enter in text)
      if (e.key === "Enter") {
        const target = e.target as HTMLElement;
        const isTextInput = target.tagName === "INPUT" && (target as HTMLInputElement).type === "text";
        if (!isTextInput || e.metaKey || e.ctrlKey) {
          e.preventDefault();
          handleSubmit();
        }
      }
    },
    [handleSubmit]
  );

  return (
    <div className="question-card" ref={cardRef} onKeyDown={handleKeyDown}>
      <div className="question-header">
        <HelpCircle size={16} className="question-icon" />
        <span className="question-title">{question.title || "Question"}</span>
        {isCrossSession && <span className="question-badge-subagent">subagent</span>}
        <span className="question-hint">Enter = submit</span>
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
        <button className="question-submit-btn" onClick={handleSubmit} aria-label="Submit answers">
          <Send size={14} />
          Submit
        </button>
      </div>
    </div>
  );
}
