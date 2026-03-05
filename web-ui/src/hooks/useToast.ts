import { useState, useCallback, useRef } from "react";

export interface Toast {
  id: number;
  message: string;
  type: "success" | "error" | "info" | "warning";
}

let nextId = 0;

/**
 * Simple toast notification system.
 * Returns { toasts, addToast, removeToast } — render ToastContainer in the layout.
 */
export function useToast() {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const timersRef = useRef<Map<number, ReturnType<typeof setTimeout>>>(new Map());

  const removeToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
    const timer = timersRef.current.get(id);
    if (timer) {
      clearTimeout(timer);
      timersRef.current.delete(id);
    }
  }, []);

  const addToast = useCallback(
    (message: string, type: Toast["type"] = "info", durationMs = 3000) => {
      const id = ++nextId;
      setToasts((prev) => [...prev, { id, message, type }]);
      const timer = setTimeout(() => removeToast(id), durationMs);
      timersRef.current.set(id, timer);
      return id;
    },
    [removeToast]
  );

  return { toasts, addToast, removeToast };
}
