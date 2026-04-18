import React from "react";
import { Mic, MicOff } from "lucide-react";
import type { VoiceStatus } from "../voice/useVoiceDictation";

interface VoiceButtonProps {
  status: VoiceStatus;
  downloadPercent: number;
  waveform: number[];
  error: string | null;
  disabled: boolean;
  onToggle: () => void;
}

const TOOLTIP: Record<VoiceStatus, string> = {
  idle: "Voice dictation (click to start)",
  loading: "Downloading speech model…",
  recording: "Listening — click to stop",
  transcribing: "Transcribing speech…",
  error: "Voice error",
};

/** SVG circular progress ring for download state. */
function ProgressRing({ percent }: { percent: number }) {
  const r = 13;
  const circ = 2 * Math.PI * r;
  const offset = circ * (1 - Math.min(percent, 100) / 100);
  return (
    <svg className="voice-progress-ring" width={32} height={32} viewBox="0 0 32 32">
      <circle cx={16} cy={16} r={r} fill="none" stroke="var(--color-border)" strokeWidth={2} />
      <circle cx={16} cy={16} r={r} fill="none" stroke="var(--color-primary)"
        strokeWidth={2.5} strokeLinecap="round" strokeDasharray={circ} strokeDashoffset={offset}
        transform="rotate(-90 16 16)" className="voice-progress-track" />
      <text x={16} y={17} textAnchor="middle" dominantBaseline="middle"
        className="voice-progress-text">{Math.round(percent)}%</text>
    </svg>
  );
}

/** Live waveform bars driven by analyser data. */
function WaveformBars({ levels }: { levels: number[] }) {
  const barW = 2;
  const gap = 1;
  const h = 18;
  const w = levels.length * (barW + gap) - gap;
  return (
    <svg className="voice-waveform" width={w} height={h} viewBox={`0 0 ${w} ${h}`}>
      {levels.map((v, i) => {
        const barH = Math.max(2, v * h);
        return (
          <rect key={i} x={i * (barW + gap)} y={(h - barH) / 2}
            width={barW} height={barH} rx={1} fill="currentColor" />
        );
      })}
    </svg>
  );
}

/** Error icon — mic with a red slash. */
function ErrorIcon() {
  return (
    <svg className="voice-error-icon" width={16} height={16} viewBox="0 0 16 16" fill="none">
      <path d="M8 1a2.5 2.5 0 0 0-2.5 2.5v4a2.5 2.5 0 0 0 5 0v-4A2.5 2.5 0 0 0 8 1Z"
        stroke="var(--color-error)" strokeWidth={1.3} />
      <path d="M4.5 7.5a3.5 3.5 0 0 0 7 0" stroke="var(--color-error)" strokeWidth={1.3}
        strokeLinecap="round" />
      <line x1={8} y1={11} x2={8} y2={14} stroke="var(--color-error)" strokeWidth={1.3}
        strokeLinecap="round" />
      {/* Diagonal red slash */}
      <line x1={3} y1={13} x2={13} y2={3} stroke="var(--color-error)" strokeWidth={1.6}
        strokeLinecap="round" />
    </svg>
  );
}

export const VoiceButton = React.memo(function VoiceButton({
  status, downloadPercent, waveform, error, disabled, onToggle,
}: VoiceButtonProps) {
  const isRecording = status === "recording";
  const isLoading = status === "loading";
  const isTranscribing = status === "transcribing";
  const isError = status === "error";
  const isBusy = isLoading || isTranscribing;

  const tooltip = isError && error ? `Error: ${error}` : TOOLTIP[status];

  const cls = [
    "prompt-btn",
    "prompt-voice-btn",
    isRecording && "voice-recording",
    isLoading && "voice-loading",
    isTranscribing && "voice-transcribing",
    isError && "voice-error",
  ].filter(Boolean).join(" ");

  return (
    <button className={cls} onClick={onToggle} disabled={disabled || isBusy}
      title={tooltip} aria-label={tooltip}>
      {isLoading ? (
        <ProgressRing percent={downloadPercent} />
      ) : isRecording ? (
        <WaveformBars levels={waveform} />
      ) : isError ? (
        <ErrorIcon />
      ) : isTranscribing ? (
        <MicOff size={16} className="spinning" />
      ) : (
        <Mic size={16} />
      )}
    </button>
  );
});
