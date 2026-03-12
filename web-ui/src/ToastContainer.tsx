import React from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { markdownComponents } from "./message-turn/CodeBlock";
import type { Toast } from "./hooks/useToast";
import { CheckCircle, XCircle, Info, AlertTriangle, X } from "lucide-react";

interface Props {
  toasts: Toast[];
  onDismiss: (id: number) => void;
}

const ICONS: Record<Toast["type"], React.ReactNode> = {
  success: <CheckCircle size={14} />,
  error: <XCircle size={14} />,
  info: <Info size={14} />,
  warning: <AlertTriangle size={14} />,
};

export function ToastContainer({ toasts, onDismiss }: Props) {
  if (toasts.length === 0) return null;

  return (
    <div className="toast-container">
      {toasts.map((toast) => (
        <div key={toast.id} className={`toast toast-${toast.type}`}>
          <span className="toast-icon">{ICONS[toast.type]}</span>
          <div className="toast-message toast-markdown">
            <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
              {toast.message}
            </ReactMarkdown>
          </div>
          <button className="toast-close" onClick={() => onDismiss(toast.id)} aria-label="Dismiss notification">
            <X size={12} />
          </button>
        </div>
      ))}
    </div>
  );
}
