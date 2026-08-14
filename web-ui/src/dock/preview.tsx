/** Dev-only harness page for the dock cards. Not part of the app bundle. */
import React from "react";
import { createRoot } from "react-dom/client";
import { QuestionDock } from "../QuestionDock";
import { PermissionDock } from "../PermissionDock";
import "../styles/index.css";

const question = {
  id: "q1", sessionID: "abc12345ff", title: "Dock appearance", time: Date.now(),
  questions: [
    { text: "The question panel used to run past the bottom of the screen. Which cap should the card use?", header: "Height", type: "select" as const,
      options: ["56dvh", "Full height", "Fixed 480px", "Content"],
      optionDescriptions: ["Body scrolls, header and footer stay put", "Lets the card grow with the content", "Same height every time", "No cap at all"] },
    { text: "Should each question get its own tab?", header: "Tabs", type: "select" as const, options: ["Yes", "No"], optionDescriptions: ["One question at a time", "Stack them all"] },
    { text: "Anything else?", header: "Notes", type: "text" as const },
  ],
};

const permission = {
  id: "p1", sessionID: "abc12345ff", toolName: "Bash", time: Date.now(),
  description: "Run a shell command in the project directory",
  metadata: { command: "cargo build --release", cwd: "/home/ubuntu/workspace/opman" },
  patterns: ["Bash(cargo build:*)"],
};

createRoot(document.getElementById("root") as HTMLElement).render(
  <div style={{ minHeight: "100dvh", background: "var(--color-bg)", display: "flex", flexDirection: "column", justifyContent: "flex-end" }}>
    <PermissionDock permissions={[permission] as never} activeSessionId="abc12345ff" onReply={() => {}} onGoToSession={() => {}} />
    <QuestionDock questions={[question, { ...question, id: "q2", title: "Second request" }] as never} activeSessionId="abc12345ff" onReply={() => {}} onDismiss={() => {}} onGoToSession={() => {}} />
  </div>
);
