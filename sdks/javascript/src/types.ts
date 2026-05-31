/** Minimal type mirror of the LSF Scene — see the JSON Schema for full spec. */
export interface LuminaScene {
  version: string;
  meta: { title: string; author: string; created_at: string };
  canvas: { width: number; height: number; fps: number; duration: number; background: string };
  assets?: { fonts?: Array<{ id: string; path: string }> };
  objects: Record<string, unknown>;
  timeline: Array<{
    time: number;
    object: string;
    state: Record<string, unknown>;
    easing?: string;
    easing_params?: number[];
  }>;
  events?: unknown[];
  camera?: unknown;
}

export interface LuminaPlayerRef {
  play(): void;
  pause(): void;
  seek(time: number): void;
  getCurrentTime(): number;
}

export interface LuminaPlayerProps {
  scene: LuminaScene;
  /** Start playing automatically when mounted. Default: true */
  autoplay?: boolean;
  /** Loop when the animation reaches its end. Default: false */
  loop?: boolean;
  /** Override canvas width for display (does not affect render resolution). */
  displayWidth?: number;
  /** Override canvas height for display. */
  displayHeight?: number;
  /** Called with the object ID when an object is clicked, or null when nothing is hit. */
  onObjectClick?: (objectId: string | null) => void;
  className?: string;
  style?: React.CSSProperties;
}
