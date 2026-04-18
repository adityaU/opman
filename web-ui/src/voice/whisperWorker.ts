/**
 * Web Worker for Whisper speech-to-text via @huggingface/transformers.
 * Loaded lazily — never included in the main bundle.
 *
 * Protocol (postMessage):
 *   Main → Worker: { type: "transcribe", audio: Float32Array }
 *   Worker → Main: { type: "status", status: "loading" | "ready" | "transcribing" }
 *   Worker → Main: { type: "progress", loaded: number, total: number, percent: number }
 *   Worker → Main: { type: "result", text: string }
 *   Worker → Main: { type: "error", message: string }
 */

import { pipeline, type AutomaticSpeechRecognitionPipeline } from "@huggingface/transformers";

let transcriber: AutomaticSpeechRecognitionPipeline | null = null;
let loading = false;

async function ensureModel(): Promise<AutomaticSpeechRecognitionPipeline> {
  if (transcriber) return transcriber;
  if (loading) throw new Error("Model already loading");
  loading = true;
  self.postMessage({ type: "status", status: "loading" });
  try {
    transcriber = await pipeline(
      "automatic-speech-recognition",
      "onnx-community/whisper-tiny.en",
      {
        dtype: "q8",
        device: "wasm",
        progress_callback: (p: Record<string, unknown>) => {
          const loaded = typeof p.loaded === "number" ? p.loaded : 0;
          const total = typeof p.total === "number" ? p.total : 1;
          const percent = typeof p.progress === "number" ? p.progress : (total > 0 ? Math.round((loaded / total) * 100) : 0);
          self.postMessage({ type: "progress", loaded, total, percent });
        },
      },
    );
    self.postMessage({ type: "status", status: "ready" });
    return transcriber;
  } finally {
    loading = false;
  }
}

self.onmessage = async (e: MessageEvent) => {
  const { type, audio } = e.data;
  if (type !== "transcribe") return;

  try {
    const model = await ensureModel();
    self.postMessage({ type: "status", status: "transcribing" });
    const result = await model(audio, { language: "en", task: "transcribe" });
    const text = Array.isArray(result) ? result.map(r => r.text).join(" ") : result.text;
    self.postMessage({ type: "result", text: text.trim() });
  } catch (err) {
    self.postMessage({ type: "error", message: err instanceof Error ? err.message : String(err) });
  }
};
