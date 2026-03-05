import React, { useState } from "react";
import type { QuestionRequest } from "./types";
import { HelpCircle, Send } from "lucide-react";

interface Props {
  questions: QuestionRequest[];
  onReply: (requestId: string, answers: string[][]) => void;
}

export function QuestionDock({ questions, onReply }: Props) {
  return (
    <div className="question-dock">
      {questions.map((q) => (
        <QuestionCard key={q.id} question={q} onReply={onReply} />
      ))}
    </div>
  );
}

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

  const handleSubmit = () => {
    onReply(question.id, answers);
  };

  const updateAnswer = (qIdx: number, value: string[]) => {
    setAnswers((prev) => {
      const next = [...prev];
      next[qIdx] = value;
      return next;
    });
  };

  return (
    <div className="question-card">
      <div className="question-header">
        <HelpCircle size={16} className="question-icon" />
        <span className="question-title">{question.title || "Question"}</span>
      </div>
      <div className="question-body">
        {question.questions.map((q, idx) => (
          <div key={idx} className="question-item">
            <label className="question-label">{q.text}</label>
            {q.type === "select" && q.options ? (
              <div className="question-options">
                {q.options.map((opt) => {
                  const selected = answers[idx]?.includes(opt);
                  return (
                    <button
                      key={opt}
                      className={`question-option ${selected ? "selected" : ""}`}
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
              <div className="question-options">
                <button
                  className={`question-option ${answers[idx]?.[0] === "yes" ? "selected" : ""}`}
                  onClick={() => updateAnswer(idx, ["yes"])}
                >
                  Yes
                </button>
                <button
                  className={`question-option ${answers[idx]?.[0] === "no" ? "selected" : ""}`}
                  onClick={() => updateAnswer(idx, ["no"])}
                >
                  No
                </button>
              </div>
            ) : (
              <input
                type="text"
                className="question-text-input"
                value={answers[idx]?.[0] || ""}
                onChange={(e) => updateAnswer(idx, [e.target.value])}
                placeholder="Type your answer..."
              />
            )}
          </div>
        ))}
      </div>
      <div className="question-actions">
        <button className="question-submit-btn" onClick={handleSubmit}>
          <Send size={14} />
          Submit
        </button>
      </div>
    </div>
  );
}
