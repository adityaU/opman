import React, { useState, useRef, useEffect, useCallback } from "react";
import type { QuestionRequest } from "./types";
import { HelpCircle, Send } from "lucide-react";

interface Props {
  questions: QuestionRequest[];
  onReply: (requestId: string, answers: string[][]) => void;
}

export const QuestionDock = React.memo(function QuestionDock({ questions, onReply }: Props) {
  return (
    <div className="question-dock" role="region" aria-label="Questions">
      {questions.map((q) => (
        <QuestionCard key={q.id} question={q} onReply={onReply} />
      ))}
    </div>
  );
});

function QuestionCard({
  question,
  onReply,
}: {
  question: QuestionRequest;
  onReply: (requestId: string, answers: string[][]) => void;
}) {
  const [answers, setAnswers] = useState<string[][]>(
    question.questions.map(() => [])
  );
  const cardRef = useRef<HTMLDivElement>(null);
  const firstInputRef = useRef<HTMLInputElement | null>(null);
  const firstButtonRef = useRef<HTMLButtonElement | null>(null);

  // Auto-focus the first interactive element when the card mounts
  useEffect(() => {
    const timer = setTimeout(() => {
      if (firstInputRef.current) {
        firstInputRef.current.focus();
      } else if (firstButtonRef.current) {
        firstButtonRef.current.focus();
      }
    }, 50);
    return () => clearTimeout(timer);
  }, [question.id]);

  const handleSubmit = useCallback(() => {
    onReply(question.id, answers);
  }, [question.id, answers, onReply]);

  const updateAnswer = (qIdx: number, value: string[]) => {
    setAnswers((prev) => {
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
        <span className="question-hint">Enter to submit</span>
      </div>
      <div className="question-body">
        {question.questions.map((q, idx) => (
          <div key={idx} className="question-item">
            <label className="question-label">{q.text}</label>
            {q.type === "select" && q.options ? (
              <div className="question-options" role="listbox" aria-label={q.text}>
                {q.options.map((opt, optIdx) => {
                  const selected = answers[idx]?.includes(opt);
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
                          updateAnswer(idx, [opt]);
                        }
                      }}
                    >
                      {opt}
                    </button>
                  );
                })}
              </div>
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
