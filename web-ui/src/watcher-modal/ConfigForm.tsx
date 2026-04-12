import React from "react";
import {
  Eye,
  Loader2,
  Play,
  Trash2,
  ChevronDown,
  Clock,
  AlertTriangle,
} from "lucide-react";
import type { WatcherConfigResponse, WatcherMessageEntry } from "../api";

interface ConfigFormProps {
  selectedId: string | null;
  continuationMsg: string;
  setContinuationMsg: (v: string) => void;
  idleTimeout: number;
  setIdleTimeout: (v: number) => void;
  includeOriginal: boolean;
  setIncludeOriginal: (v: boolean) => void;
  originalMessage: string | null;
  setOriginalMessage: (v: string | null) => void;
  hangMessage: string;
  setHangMessage: (v: string) => void;
  hangTimeout: number;
  setHangTimeout: (v: number) => void;
  existingWatcher: WatcherConfigResponse | null;
  error: string | null;
  saving: boolean;
  showMsgPicker: boolean;
  setShowMsgPicker: (v: boolean) => void;
  userMessages: WatcherMessageEntry[];
  loadingMsgs: boolean;
  loadMessages: () => void;
  handleSave: () => void;
  handleDelete: () => void;
  formRef: React.RefObject<HTMLDivElement | null> | React.RefObject<HTMLDivElement>;
}

export function ConfigForm({
  selectedId,
  continuationMsg,
  setContinuationMsg,
  idleTimeout,
  setIdleTimeout,
  includeOriginal,
  setIncludeOriginal,
  originalMessage,
  setOriginalMessage,
  hangMessage,
  setHangMessage,
  hangTimeout,
  setHangTimeout,
  existingWatcher,
  error,
  saving,
  showMsgPicker,
  setShowMsgPicker,
  userMessages,
  loadingMsgs,
  loadMessages,
  handleSave,
  handleDelete,
  formRef,
}: ConfigFormProps) {
  return (
    <div className="watcher-config-form" ref={formRef as React.RefObject<HTMLDivElement>}>
      {!selectedId ? (
        <div className="watcher-empty">Select a session to configure</div>
      ) : (
        <>
          <div className="watcher-form-section">
            <label className="watcher-form-label">Continuation Message</label>
            <textarea
              className="watcher-form-textarea"
              value={continuationMsg}
              onChange={(e) => setContinuationMsg(e.target.value)}
              rows={3}
              placeholder="Message sent when session goes idle..."
            />
          </div>
          <div className="watcher-form-section">
            <label className="watcher-form-label">
              <Clock size={12} />
              Idle Timeout (seconds)
            </label>
            <input
              type="number"
              className="watcher-form-input"
              value={idleTimeout}
              onChange={(e) =>
                setIdleTimeout(Math.max(1, parseInt(e.target.value) || 1))
              }
              min={1}
            />
            <span className="watcher-form-hint">
              Seconds to wait after session goes idle before sending continuation
            </span>
          </div>

          <div className="watcher-form-section">
            <label className="watcher-form-check watcher-form-label">
              <input
                type="checkbox"
                checked={includeOriginal}
                onChange={(e) => setIncludeOriginal(e.target.checked)}
              />
              Include original message
            </label>
            {includeOriginal && (
              <OriginalMessageSection
                originalMessage={originalMessage}
                setOriginalMessage={setOriginalMessage}
                showMsgPicker={showMsgPicker}
                setShowMsgPicker={setShowMsgPicker}
                userMessages={userMessages}
                loadingMsgs={loadingMsgs}
                loadMessages={loadMessages}
              />
            )}
          </div>

          <div className="watcher-form-divider" />

          <div className="watcher-form-section">
            <label className="watcher-form-label">
              <AlertTriangle size={12} />
              Hang Detection Message
            </label>
            <textarea
              className="watcher-form-textarea"
              value={hangMessage}
              onChange={(e) => setHangMessage(e.target.value)}
              rows={2}
              placeholder="Message sent when session appears hung..."
            />
          </div>

          <div className="watcher-form-section">
            <label className="watcher-form-label">
              <Clock size={12} />
              Hang Timeout (seconds)
            </label>
            <input
              type="number"
              className="watcher-form-input"
              value={hangTimeout}
              onChange={(e) =>
                setHangTimeout(Math.max(10, parseInt(e.target.value) || 10))
              }
              min={10}
            />
            <span className="watcher-form-hint">
              Seconds of silence before session is considered hung
            </span>
          </div>

          {error && <div className="watcher-form-error">{error}</div>}

          <FormActions
            existingWatcher={existingWatcher}
            saving={saving}
            continuationMsg={continuationMsg}
            handleSave={handleSave}
            handleDelete={handleDelete}
          />
        </>
      )}
    </div>
  );
}

function OriginalMessageSection({
  originalMessage,
  setOriginalMessage,
  showMsgPicker,
  setShowMsgPicker,
  userMessages,
  loadingMsgs,
  loadMessages,
}: {
  originalMessage: string | null;
  setOriginalMessage: (v: string | null) => void;
  showMsgPicker: boolean;
  setShowMsgPicker: (v: boolean) => void;
  userMessages: WatcherMessageEntry[];
  loadingMsgs: boolean;
  loadMessages: () => void;
}) {
  return (
    <div className="watcher-original-msg">
      {originalMessage ? (
        <div className="watcher-original-preview">
          <span className="watcher-original-text">
            {originalMessage.length > 120
              ? originalMessage.slice(0, 120) + "..."
              : originalMessage}
          </span>
          <button
            className="watcher-btn-sm"
            onClick={() => setOriginalMessage(null)}
          >
            Clear
          </button>
        </div>
      ) : (
        <button
          className="watcher-btn-sm"
          onClick={loadMessages}
          disabled={loadingMsgs}
        >
          {loadingMsgs ? (
            <Loader2 size={11} className="spinning" />
          ) : (
            <ChevronDown size={11} />
          )}
          Pick from session messages
        </button>
      )}
      {showMsgPicker && (
        <div className="watcher-msg-picker">
          {userMessages.length === 0 ? (
            <div className="watcher-msg-picker-empty">
              No user messages found
            </div>
          ) : (
            userMessages.slice(0, 10).map((m, i) => (
              <button
                key={i}
                className="watcher-msg-picker-item"
                onClick={() => {
                  setOriginalMessage(m.text);
                  setShowMsgPicker(false);
                }}
              >
                {m.text.length > 100 ? m.text.slice(0, 100) + "..." : m.text}
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
}

function FormActions({
  existingWatcher,
  saving,
  continuationMsg,
  handleSave,
  handleDelete,
}: {
  existingWatcher: WatcherConfigResponse | null;
  saving: boolean;
  continuationMsg: string;
  handleSave: () => void;
  handleDelete: () => void;
}) {
  return (
    <div className="watcher-form-actions">
      {existingWatcher ? (
        <>
          <button
            className="watcher-btn watcher-btn-primary"
            onClick={handleSave}
            disabled={saving || !continuationMsg.trim()}
          >
            {saving ? (
              <Loader2 size={13} className="spinning" />
            ) : (
              <Play size={13} />
            )}
            Update Watcher
          </button>
          <button
            className="watcher-btn watcher-btn-danger"
            onClick={handleDelete}
            disabled={saving}
          >
            <Trash2 size={13} />
            Remove
          </button>
        </>
      ) : (
        <button
          className="watcher-btn watcher-btn-primary"
          onClick={handleSave}
          disabled={saving || !continuationMsg.trim()}
        >
          {saving ? (
            <Loader2 size={13} className="spinning" />
          ) : (
            <Eye size={13} />
          )}
          Start Watcher
        </button>
      )}
    </div>
  );
}
