import { useEffect, useRef, useState } from 'react';
import init, { LuminaEngine } from '../wasm/lumina_wasm';
import type { LuminaScene } from './types';

export interface UseLuminaResult {
  /** Raw RGBA pixel data for the current frame, or null while loading. */
  frameData: Uint8Array | null;
  /** Seek to a specific time (seconds). */
  seek: (time: number) => void;
  /** Whether the WASM engine is ready. */
  ready: boolean;
  width: number;
  height: number;
  duration: number;
}

/**
 * Low-level hook for full control over rendering.
 * Useful when you need to draw into your own canvas or texture.
 *
 * @example
 * ```ts
 * const { frameData, seek, width, height } = useLumina(scene);
 * ```
 */
export function useLumina(scene: LuminaScene): UseLuminaResult {
  const engineRef = useRef<LuminaEngine | null>(null);
  const [ready, setReady] = useState(false);
  const [frameData, setFrameData] = useState<Uint8Array | null>(null);
  const [time, setTime] = useState(0);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      await init();
      if (cancelled) return;
      engineRef.current = new LuminaEngine(scene as object);
      setReady(true);
    })();
    return () => { cancelled = true; };
  }, [scene]);

  useEffect(() => {
    if (!ready || !engineRef.current) return;
    const data = engineRef.current.render_frame(time);
    setFrameData(new Uint8Array(data));
  }, [ready, time]);

  return {
    frameData,
    seek: setTime,
    ready,
    width: ready && engineRef.current ? engineRef.current.width() : scene.canvas.width,
    height: ready && engineRef.current ? engineRef.current.height() : scene.canvas.height,
    duration: ready && engineRef.current ? engineRef.current.duration() : scene.canvas.duration,
  };
}
