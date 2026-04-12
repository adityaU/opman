import { X, GitPullRequest, Sparkles } from "lucide-react";
import type { PRModalData } from "../types";

interface Props {
  data: PRModalData;
  onClose: () => void;
}

export function PRModal({ data, onClose }: Props) {
  return (
    <div className="pr-description-modal-overlay" onClick={onClose}>
      <div className="pr-description-modal" onClick={(e) => e.stopPropagation()}>
        <div className="pr-description-modal-header">
          <GitPullRequest size={16} />
          <span>PR Context: {data.branch} → {data.base}</span>
          <button className="pr-description-modal-close" onClick={onClose}><X size={14} /></button>
        </div>
        <div className="pr-description-modal-body">
          <div className="pr-description-stats">
            <span>{data.commits.length} commits</span>
            <span>{data.files_changed} files changed</span>
          </div>
          <div className="pr-description-commits">
            <h4>Commits</h4>
            <ul>
              {data.commits.map((c) => (
                <li key={c.hash}><code>{c.hash.slice(0, 7)}</code> {c.message}</li>
              ))}
            </ul>
          </div>
          <p className="pr-description-hint">
            <Sparkles size={12} /> AI is generating a PR description in the chat panel...
          </p>
        </div>
      </div>
    </div>
  );
}
