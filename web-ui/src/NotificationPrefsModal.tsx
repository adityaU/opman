import React, { useState, useRef, useCallback } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import {
  loadNotificationPrefs,
  saveNotificationPrefs,
  requestNotificationPermission,
  canNotify,
} from "./NotificationManager";
import type { NotificationPrefs } from "./NotificationManager";
import { X, Bell, BellOff, CheckCircle, FileCode, Shield, HelpCircle, Activity, Zap } from "lucide-react";

interface Props {
  onClose: () => void;
}

interface PrefItem {
  key: keyof NotificationPrefs;
  label: string;
  description: string;
  icon: React.ReactNode;
}

const ITEMS: PrefItem[] = [
  {
    key: "session_complete",
    label: "Session Complete",
    description: "Notify when a session finishes processing",
    icon: <CheckCircle size={14} />,
  },
  {
    key: "permission_request",
    label: "Permission Request",
    description: "Notify when AI needs tool approval",
    icon: <Shield size={14} />,
  },
  {
    key: "question",
    label: "AI Question",
    description: "Notify when AI asks a question",
    icon: <HelpCircle size={14} />,
  },
  {
    key: "watcher_trigger",
    label: "Watcher Trigger",
    description: "Notify when a watcher auto-continues",
    icon: <Activity size={14} />,
  },
  {
    key: "file_edit",
    label: "File Edits",
    description: "Notify on each file edit (can be noisy)",
    icon: <FileCode size={14} />,
  },
];

export function NotificationPrefsModal({ onClose }: Props) {
  const [prefs, setPrefs] = useState<NotificationPrefs>(loadNotificationPrefs);
  const [permissionGranted, setPermissionGranted] = useState(canNotify());
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  const toggle = useCallback((key: keyof NotificationPrefs) => {
    setPrefs((prev) => {
      const next = { ...prev, [key]: !prev[key] };
      saveNotificationPrefs(next);
      return next;
    });
  }, []);

  const handleRequestPermission = useCallback(async () => {
    const granted = await requestNotificationPermission();
    setPermissionGranted(granted);
    if (granted) {
      setPrefs((prev) => {
        const next = { ...prev, enabled: true };
        saveNotificationPrefs(next);
        return next;
      });
    }
  }, []);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="notification-prefs-modal"
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="notification-prefs-header">
          <h3>Notification Preferences</h3>
          <button onClick={onClose} aria-label="Close">
            <X size={14} />
          </button>
        </div>

        <div className="notification-prefs-body">
          {/* Browser permission status */}
          <div className="notification-prefs-permission">
            {permissionGranted ? (
              <div className="notification-prefs-permission-info">
                <span className="notification-prefs-permission-label">Browser Permission</span>
                <span className="notification-prefs-status granted">
                  <Bell size={14} /> Browser notifications allowed
                </span>
              </div>
            ) : (
              <div className="notification-prefs-status denied">
                <BellOff size={14} />
                <span>Browser notifications not enabled</span>
                <button className="notification-prefs-grant-btn" onClick={handleRequestPermission}>
                  Enable
                </button>
              </div>
            )}
          </div>

          {/* Master toggle */}
          <div
            className="notification-prefs-item master"
            onClick={() => toggle("enabled")}
          >
            <div className="notification-prefs-item-left">
              <Zap size={14} />
              <div>
                <div className="notification-prefs-item-label">All Notifications</div>
                <div className="notification-prefs-item-desc">Master toggle for all notification types</div>
              </div>
            </div>
            <span className={`notification-prefs-badge ${prefs.enabled ? "on" : "off"}`}>
              {prefs.enabled ? "ON" : "OFF"}
            </span>
          </div>

          {/* Individual toggles */}
          {ITEMS.map((item) => (
            <div
              key={item.key}
              className={`notification-prefs-item ${!prefs.enabled ? "disabled" : ""}`}
              onClick={() => prefs.enabled && toggle(item.key)}
            >
              <div className="notification-prefs-item-left">
                {item.icon}
                <div>
                  <div className="notification-prefs-item-label">{item.label}</div>
                  <div className="notification-prefs-item-desc">{item.description}</div>
                </div>
              </div>
              <span className={`notification-prefs-badge ${prefs[item.key] ? "on" : "off"}`}>
                {prefs[item.key] ? "ON" : "OFF"}
              </span>
            </div>
          ))}
        </div>

        <div className="notification-prefs-footer">
          Notifications are shown when the tab is in the background.
        </div>
      </div>
    </div>
  );
}
