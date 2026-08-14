import React from "react";
import type { QuestionItem } from "../types";

interface Props {
  readonly question: QuestionItem;
  /** Currently selected option labels (or the single typed value for text questions). */
  readonly answer: readonly string[];
  readonly customText: string;
  readonly onAnswer: (value: string[]) => void;
  readonly onCustomText: (value: string) => void;
  /** Focus target for the first control of the active question. */
  readonly buttonRef?: React.Ref<HTMLButtonElement>;
  readonly inputRef?: React.Ref<HTMLInputElement>;
}

/** One question's input surface: option list, yes/no, or a free-text field. */
export const QuestionField = React.memo(function QuestionField(props: Props) {
  const { question, answer } = props;

  if (question.type === "select" && question.options) {
    return (
      <>
        <div className="dock-options" role="listbox" aria-label={question.text}>
          {question.options.map((option, index) => (
            <OptionButton
              key={option}
              label={option}
              description={question.optionDescriptions?.[index]}
              selected={answer.includes(option)}
              buttonRef={index === 0 ? props.buttonRef : undefined}
              onClick={() => {
                if (question.multiple) {
                  props.onAnswer(
                    answer.includes(option)
                      ? answer.filter((value) => value !== option)
                      : [...answer, option],
                  );
                  return;
                }
                props.onCustomText("");
                props.onAnswer([option]);
              }}
            />
          ))}
        </div>
        {question.custom !== false && (
          <input
            ref={props.inputRef}
            type="text"
            className="dock-input dock-input--custom"
            value={props.customText}
            onChange={(event) => {
              props.onCustomText(event.target.value);
              if (event.target.value) props.onAnswer([]);
            }}
            placeholder="Type your own answer..."
            aria-label={`Custom answer for: ${question.text}`}
          />
        )}
      </>
    );
  }

  if (question.type === "confirm") {
    return (
      <div className="dock-options" role="group" aria-label={question.text}>
        <button
          type="button"
          ref={props.buttonRef}
          className={`dock-option${answer[0] === "yes" ? " dock-option--on" : ""}`}
          onClick={() => props.onAnswer(["yes"])}
          aria-pressed={answer[0] === "yes"}
        >
          Yes
        </button>
        <button
          type="button"
          className={`dock-option${answer[0] === "no" ? " dock-option--on" : ""}`}
          onClick={() => props.onAnswer(["no"])}
          aria-pressed={answer[0] === "no"}
        >
          No
        </button>
      </div>
    );
  }

  return (
    <input
      ref={props.inputRef}
      type="text"
      className="dock-input"
      value={answer[0] || ""}
      onChange={(event) => props.onAnswer([event.target.value])}
      placeholder="Type your answer..."
      aria-label={question.text}
    />
  );
});

function OptionButton({
  label,
  description,
  selected,
  onClick,
  buttonRef,
}: {
  label: string;
  description?: string;
  selected: boolean;
  onClick: () => void;
  buttonRef?: React.Ref<HTMLButtonElement>;
}) {
  return (
    <button
      type="button"
      ref={buttonRef}
      className={`dock-option${selected ? " dock-option--on" : ""}`}
      role="option"
      aria-selected={selected}
      onClick={onClick}
    >
      <span className="dock-option-label">{label}</span>
      {description && <span className="dock-option-desc">{description}</span>}
    </button>
  );
}
