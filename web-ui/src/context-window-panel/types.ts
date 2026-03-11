export interface ContextWindowPanelProps {
  onClose: () => void;
  sessionId: string | null;
  /** Callback when compact is triggered (to show toast) */
  onCompact?: () => void;
}

/** SVG gauge constants */
export const GAUGE_RADIUS = 54;
export const GAUGE_CIRCUMFERENCE = 2 * Math.PI * GAUGE_RADIUS;
