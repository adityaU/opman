/**
 * Hook for voice dictation via Whisper (web worker).
 * Desktop only (>= 768px viewport). Lazy-loads the worker on first use.
 *
 * Features:
 * - Real-time STT: records in chunks, transcribes each chunk as it arrives
 * - Silence detection: auto-stops after sustained silence
 * - Duplicate detection: filters out repeated transcriptions
 * - Download progress: reports model download percentage
 * - Waveform data: provides live audio levels for visualization
 */

import { useState, useRef, useCallback, useEffect } from "react";

export type VoiceStatus = "idle" | "loading" | "recording" | "transcribing" | "error";

export interface VoiceDictation {
  status: VoiceStatus;
  isDesktop: boolean;
  error: string | null;
  /** 0–100 model download progress (only meaningful during "loading") */
  downloadPercent: number;
  /** Live waveform levels (0–1) array of recent amplitudes, updated ~60fps */
  waveform: number[];
  toggle: () => void;
}

/** Resample AudioBuffer to 16kHz mono Float32Array (Whisper requirement). */
async function resampleTo16kHz(buffer: AudioBuffer): Promise<Float32Array> {
  const ctx = new OfflineAudioContext(1, Math.ceil(buffer.duration * 16000), 16000);
  const src = ctx.createBufferSource();
  src.buffer = buffer;
  src.connect(ctx.destination);
  src.start();
  const out = await ctx.startRendering();
  return out.getChannelData(0);
}

// ── Constants ──────────────────────────────────────────────────
const CHUNK_MS = 4000;            // Send audio chunks every 4s for real-time STT
const SILENCE_THRESHOLD = 0.01;   // RMS below this = silence
const SILENCE_TIMEOUT_MS = 3000;  // Auto-stop after 3s continuous silence
const WAVEFORM_BARS = 24;         // Number of bars in the waveform display
const DUPLICATE_WINDOW = 3;       // Compare against last N transcripts

export function useVoiceDictation(onTranscript: (text: string) => void): VoiceDictation {
  const [status, setStatus] = useState<VoiceStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const [downloadPercent, setDownloadPercent] = useState(0);
  const [waveform, setWaveform] = useState<number[]>(() => new Array(WAVEFORM_BARS).fill(0));

  const workerRef = useRef<Worker | null>(null);
  const mediaRef = useRef<MediaRecorder | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const audioCtxRef = useRef<AudioContext | null>(null);
  const rafRef = useRef(0);
  const silenceStartRef = useRef(0);
  const recentTranscriptsRef = useRef<string[]>([]);
  const pendingChunksRef = useRef(0);
  const isRecordingRef = useRef(false);
  const isDesktop = typeof window !== "undefined" && window.innerWidth >= 768;

  // ── Waveform analyser loop ─────────────────────────────────
  const updateWaveform = useCallback(() => {
    const analyser = analyserRef.current;
    if (!analyser || !isRecordingRef.current) return;
    const data = new Uint8Array(analyser.frequencyBinCount);
    analyser.getByteFrequencyData(data);

    // Downsample to WAVEFORM_BARS buckets, normalize to 0–1
    const step = Math.floor(data.length / WAVEFORM_BARS);
    const bars: number[] = [];
    for (let i = 0; i < WAVEFORM_BARS; i++) {
      let sum = 0;
      for (let j = 0; j < step; j++) sum += data[i * step + j];
      bars.push(sum / (step * 255));
    }
    setWaveform(bars);

    // Silence detection via RMS of time-domain data
    const timeData = new Float32Array(analyser.fftSize);
    analyser.getFloatTimeDomainData(timeData);
    let rms = 0;
    for (let i = 0; i < timeData.length; i++) rms += timeData[i] * timeData[i];
    rms = Math.sqrt(rms / timeData.length);

    if (rms < SILENCE_THRESHOLD) {
      if (!silenceStartRef.current) silenceStartRef.current = Date.now();
      else if (Date.now() - silenceStartRef.current > SILENCE_TIMEOUT_MS) {
        // Auto-stop on sustained silence
        stopRecording();
        return;
      }
    } else {
      silenceStartRef.current = 0;
    }

    rafRef.current = requestAnimationFrame(updateWaveform);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // ── Worker management ──────────────────────────────────────
  const getWorker = useCallback((): Worker => {
    if (workerRef.current) return workerRef.current;
    const w = new Worker(new URL("./whisperWorker.ts", import.meta.url), { type: "module" });
    w.onmessage = (e: MessageEvent) => {
      const { type, text, message, percent } = e.data;
      if (type === "status") {
        if (e.data.status === "loading") setStatus(prev => prev === "recording" ? prev : "loading");
      } else if (type === "progress") {
        setDownloadPercent(typeof percent === "number" ? Math.min(percent, 100) : 0);
      } else if (type === "result") {
        pendingChunksRef.current = Math.max(0, pendingChunksRef.current - 1);
        if (text && !isDuplicate(text)) {
          recentTranscriptsRef.current.push(text);
          if (recentTranscriptsRef.current.length > DUPLICATE_WINDOW) {
            recentTranscriptsRef.current.shift();
          }
          onTranscript(text);
        }
        // Only go idle if no more pending chunks and not recording
        if (pendingChunksRef.current <= 0 && !isRecordingRef.current) {
          setStatus("idle");
        }
      } else if (type === "error") {
        pendingChunksRef.current = Math.max(0, pendingChunksRef.current - 1);
        setError(message);
        setStatus("error");
        setTimeout(() => setStatus("idle"), 3000);
      }
    };
    workerRef.current = w;
    return w;
  }, [onTranscript]);

  // ── Duplicate detection ────────────────────────────────────
  function isDuplicate(text: string): boolean {
    const normalized = text.toLowerCase().trim().replace(/[.,!?]+$/, "");
    if (!normalized || normalized.length < 3) return true; // skip noise
    return recentTranscriptsRef.current.some(prev => {
      const prevNorm = prev.toLowerCase().trim().replace(/[.,!?]+$/, "");
      return prevNorm === normalized || prevNorm.includes(normalized) || normalized.includes(prevNorm);
    });
  }

  // ── Send chunk to worker ───────────────────────────────────
  const transcribeChunk = useCallback(async (blob: Blob) => {
    if (blob.size < 1000) return; // skip tiny chunks
    try {
      const arrayBuf = await blob.arrayBuffer();
      const ctx = new AudioContext({ sampleRate: 16000 });
      const decoded = await ctx.decodeAudioData(arrayBuf);
      await ctx.close();
      const float32 = await resampleTo16kHz(decoded);
      pendingChunksRef.current++;
      const worker = getWorker();
      worker.postMessage({ type: "transcribe", audio: float32 }, [float32.buffer]);
    } catch {
      // Chunk too small or corrupt — skip silently
    }
  }, [getWorker]);

  // ── Stop recording ─────────────────────────────────────────
  const stopRecording = useCallback(() => {
    isRecordingRef.current = false;
    cancelAnimationFrame(rafRef.current);
    setWaveform(new Array(WAVEFORM_BARS).fill(0));

    const recorder = mediaRef.current;
    if (recorder && recorder.state !== "inactive") recorder.stop();
    mediaRef.current = null;

    // Stop mic tracks
    streamRef.current?.getTracks().forEach(t => t.stop());
    streamRef.current = null;

    // Clean up audio context
    if (audioCtxRef.current) {
      audioCtxRef.current.close().catch(() => {});
      audioCtxRef.current = null;
      analyserRef.current = null;
    }

    if (pendingChunksRef.current > 0) {
      setStatus("transcribing");
    } else {
      setStatus("idle");
    }
  }, []);

  // ── Start recording ────────────────────────────────────────
  const startRecording = useCallback(async () => {
    setError(null);
    silenceStartRef.current = 0;
    recentTranscriptsRef.current = [];
    pendingChunksRef.current = 0;

    // Pre-warm worker (starts model download if needed)
    getWorker();

    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      streamRef.current = stream;

      // Set up analyser for waveform + silence detection
      const actx = new AudioContext();
      audioCtxRef.current = actx;
      const source = actx.createMediaStreamSource(stream);
      const analyser = actx.createAnalyser();
      analyser.fftSize = 256;
      analyser.smoothingTimeConstant = 0.7;
      source.connect(analyser);
      analyserRef.current = analyser;

      // Start MediaRecorder with chunked output for real-time STT
      const recorder = new MediaRecorder(stream, { mimeType: "audio/webm;codecs=opus" });
      let chunks: Blob[] = [];

      recorder.ondataavailable = (e) => {
        if (e.data.size > 0) chunks.push(e.data);
      };

      recorder.onstop = () => {
        // Transcribe any remaining chunks
        if (chunks.length > 0) {
          const blob = new Blob(chunks, { type: "audio/webm" });
          chunks = [];
          transcribeChunk(blob);
        }
      };

      mediaRef.current = recorder;
      isRecordingRef.current = true;
      recorder.start(CHUNK_MS);

      // Periodically flush chunks for real-time transcription
      const interval = setInterval(() => {
        if (!isRecordingRef.current) { clearInterval(interval); return; }
        if (chunks.length > 0) {
          const blob = new Blob(chunks, { type: "audio/webm" });
          chunks = [];
          transcribeChunk(blob);
        }
      }, CHUNK_MS);

      setStatus("recording");
      rafRef.current = requestAnimationFrame(updateWaveform);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Microphone access denied");
      setStatus("error");
      setTimeout(() => setStatus("idle"), 3000);
    }
  }, [getWorker, transcribeChunk, updateWaveform]);

  // ── Toggle ─────────────────────────────────────────────────
  const toggle = useCallback(() => {
    if (status === "recording") {
      stopRecording();
    } else if (status === "idle" || status === "error") {
      startRecording();
    }
    // loading / transcribing — ignore clicks
  }, [status, startRecording, stopRecording]);

  // Cleanup on unmount
  useEffect(() => () => {
    isRecordingRef.current = false;
    cancelAnimationFrame(rafRef.current);
    mediaRef.current?.stop();
    streamRef.current?.getTracks().forEach(t => t.stop());
    audioCtxRef.current?.close().catch(() => {});
  }, []);

  return { status, isDesktop, error, downloadPercent, waveform, toggle };
}
