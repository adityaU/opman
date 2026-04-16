/**
 * Hook for voice dictation via Whisper (web worker).
 * Desktop only (>= 768px viewport). Lazy-loads the worker on first use.
 */

import { useState, useRef, useCallback } from "react";

export type VoiceStatus = "idle" | "loading" | "recording" | "transcribing" | "error";

interface VoiceDictation {
  status: VoiceStatus;
  isDesktop: boolean;
  error: string | null;
  toggle: () => void;
}

/** Resample AudioBuffer to 16kHz mono Float32Array (Whisper requirement). */
function resampleTo16kHz(buffer: AudioBuffer): Float32Array {
  const ctx = new OfflineAudioContext(1, Math.ceil(buffer.duration * 16000), 16000);
  const src = ctx.createBufferSource();
  src.buffer = buffer;
  src.connect(ctx.destination);
  src.start();
  return ctx.startRendering().then(out => out.getChannelData(0)) as unknown as Float32Array;
}

export function useVoiceDictation(onTranscript: (text: string) => void): VoiceDictation {
  const [status, setStatus] = useState<VoiceStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const workerRef = useRef<Worker | null>(null);
  const mediaRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const isDesktop = typeof window !== "undefined" && window.innerWidth >= 768;

  const getWorker = useCallback((): Worker => {
    if (workerRef.current) return workerRef.current;
    const w = new Worker(new URL("./whisperWorker.ts", import.meta.url), { type: "module" });
    w.onmessage = (e: MessageEvent) => {
      const { type, status: s, text, message } = e.data;
      if (type === "status") {
        if (s === "loading") setStatus("loading");
        else if (s === "transcribing") setStatus("transcribing");
        // "ready" — no status change needed (recording will set its own)
      } else if (type === "result") {
        if (text) onTranscript(text);
        setStatus("idle");
      } else if (type === "error") {
        setError(message);
        setStatus("error");
        setTimeout(() => setStatus("idle"), 3000);
      }
    };
    workerRef.current = w;
    return w;
  }, [onTranscript]);

  const stopRecording = useCallback(async () => {
    const recorder = mediaRef.current;
    if (!recorder || recorder.state === "inactive") return;
    return new Promise<void>(resolve => {
      recorder.onstop = async () => {
        const blob = new Blob(chunksRef.current, { type: "audio/webm" });
        chunksRef.current = [];
        mediaRef.current = null;
        // Stop all mic tracks
        recorder.stream.getTracks().forEach(t => t.stop());
        try {
          const arrayBuf = await blob.arrayBuffer();
          const audioCtx = new AudioContext({ sampleRate: 16000 });
          const decoded = await audioCtx.decodeAudioData(arrayBuf);
          await audioCtx.close();
          const float32 = await resampleTo16kHz(decoded);
          const worker = getWorker();
          worker.postMessage({ type: "transcribe", audio: float32 }, [float32.buffer]);
        } catch (err) {
          setError(err instanceof Error ? err.message : "Audio decode failed");
          setStatus("error");
          setTimeout(() => setStatus("idle"), 3000);
        }
        resolve();
      };
      recorder.stop();
    });
  }, [getWorker]);

  const startRecording = useCallback(async () => {
    setError(null);
    // Pre-warm worker (starts model download if needed)
    getWorker();
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const recorder = new MediaRecorder(stream, { mimeType: "audio/webm;codecs=opus" });
      chunksRef.current = [];
      recorder.ondataavailable = (e) => { if (e.data.size > 0) chunksRef.current.push(e.data); };
      mediaRef.current = recorder;
      recorder.start();
      setStatus("recording");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Microphone access denied");
      setStatus("error");
      setTimeout(() => setStatus("idle"), 3000);
    }
  }, [getWorker]);

  const toggle = useCallback(() => {
    if (status === "recording") {
      stopRecording();
    } else if (status === "idle" || status === "error") {
      startRecording();
    }
    // loading / transcribing — ignore clicks
  }, [status, startRecording, stopRecording]);

  return { status, isDesktop, error, toggle };
}
