import init, { LuminaEngine } from '../wasm/lumina_wasm';
import type { LuminaScene } from './types';

export interface VanillaPlayerHandle {
  play(): void;
  pause(): void;
  seek(time: number): void;
  destroy(): void;
  getCurrentTime(): number;
}

/**
 * Vanilla-JS (framework-free) player factory. Attaches to an existing canvas.
 *
 * @example
 * ```js
 * import { createPlayer } from '@lumina/sdk';
 *
 * const player = await createPlayer(document.getElementById('my-canvas'), scene);
 * player.play();
 * ```
 */
export async function createPlayer(
  canvas: HTMLCanvasElement,
  scene: LuminaScene,
  options: { autoplay?: boolean; loop?: boolean } = {}
): Promise<VanillaPlayerHandle> {
  await init();
  const engine = new LuminaEngine(scene as object);

  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('Canvas 2D context unavailable');

  canvas.width = engine.width();
  canvas.height = engine.height();

  const duration = engine.duration();
  const { autoplay = false, loop = false } = options;

  let rafId: number | null = null;
  let playing = false;
  let startMs: number | null = null;
  let pausedAt = 0;

  function drawAt(t: number) {
    const rgba = engine.render_frame(Math.max(0, Math.min(t, duration)));
    ctx!.putImageData(new ImageData(new Uint8ClampedArray(rgba), canvas.width, canvas.height), 0, 0);
  }

  function tick(now: number) {
    if (!playing) return;
    if (startMs === null) startMs = now;
    let t = (now - startMs) / 1000 + pausedAt;
    if (t >= duration) {
      if (loop) { startMs = now; pausedAt = 0; t = 0; }
      else { t = duration; playing = false; }
    }
    drawAt(t);
    if (playing) rafId = requestAnimationFrame(tick);
  }

  drawAt(0);
  if (autoplay) {
    playing = true;
    rafId = requestAnimationFrame(tick);
  }

  return {
    play() {
      if (playing) return;
      playing = true;
      startMs = null;
      rafId = requestAnimationFrame(tick);
    },
    pause() {
      if (!playing) return;
      if (startMs !== null) pausedAt += (performance.now() - startMs) / 1000;
      playing = false;
      if (rafId !== null) cancelAnimationFrame(rafId);
    },
    seek(time: number) {
      pausedAt = Math.max(0, time);
      startMs = null;
      drawAt(pausedAt);
    },
    getCurrentTime() {
      if (!playing || startMs === null) return pausedAt;
      return (performance.now() - startMs) / 1000 + pausedAt;
    },
    destroy() {
      playing = false;
      if (rafId !== null) cancelAnimationFrame(rafId);
    },
  };
}
