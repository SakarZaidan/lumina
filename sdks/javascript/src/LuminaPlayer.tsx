import React, {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from 'react';
import init, { LuminaEngine } from '../wasm/lumina_wasm';
import type { LuminaPlayerProps, LuminaPlayerRef } from './types';

let wasmInitialized = false;
let wasmInitPromise: Promise<void> | null = null;

function ensureWasm(): Promise<void> {
  if (wasmInitialized) return Promise.resolve();
  if (!wasmInitPromise) {
    wasmInitPromise = init().then(() => { wasmInitialized = true; });
  }
  return wasmInitPromise;
}

/**
 * Drop-in React component that renders a Lumina scene onto a <canvas>.
 *
 * @example
 * ```tsx
 * import { LuminaPlayer } from '@lumina/sdk';
 *
 * <LuminaPlayer scene={myScene} autoplay loop />
 * ```
 */
export const LuminaPlayer = forwardRef<LuminaPlayerRef, LuminaPlayerProps>(
  function LuminaPlayer(props, ref) {
    const {
      scene,
      autoplay = true,
      loop = false,
      displayWidth,
      displayHeight,
      onObjectClick,
      className,
      style,
    } = props;

    const canvasRef = useRef<HTMLCanvasElement>(null);
    const engineRef = useRef<LuminaEngine | null>(null);
    const rafRef = useRef<number | null>(null);
    const startTimeRef = useRef<number | null>(null);
    const pausedAtRef = useRef<number>(0);
    const playingRef = useRef(false);

    const [ready, setReady] = useState(false);

    // Initialise WASM + engine once per scene
    useEffect(() => {
      let cancelled = false;
      (async () => {
        await ensureWasm();
        if (cancelled) return;
        const engine = new LuminaEngine(scene as object);
        engineRef.current = engine;
        setReady(true);
      })();
      return () => { cancelled = true; };
    }, [scene]);

    // Render loop
    useEffect(() => {
      if (!ready) return;
      const canvas = canvasRef.current;
      const engine = engineRef.current;
      if (!canvas || !engine) return;

      const ctx = canvas.getContext('2d');
      if (!ctx) return;

      const duration = engine.duration();
      const w = engine.width();
      const h = engine.height();

      function drawFrame(t: number) {
        if (!ctx || !engine) return;
        const clamped = Math.max(0, Math.min(t, duration));
        const rgba = engine.render_frame(clamped);
        const imageData = new ImageData(new Uint8ClampedArray(rgba), w, h);
        ctx.putImageData(imageData, 0, 0);
      }

      function tick(now: number) {
        if (!playingRef.current) return;
        if (startTimeRef.current === null) startTimeRef.current = now;
        let t = (now - startTimeRef.current) / 1000 + pausedAtRef.current;
        if (t >= duration) {
          if (loop) {
            startTimeRef.current = now;
            pausedAtRef.current = 0;
            t = 0;
          } else {
            t = duration;
            playingRef.current = false;
          }
        }
        drawFrame(t);
        if (playingRef.current) {
          rafRef.current = requestAnimationFrame(tick);
        }
      }

      // Draw initial frame
      drawFrame(pausedAtRef.current);

      if (autoplay) {
        playingRef.current = true;
        startTimeRef.current = null;
        rafRef.current = requestAnimationFrame(tick);
      }

      return () => {
        if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
        playingRef.current = false;
      };
    }, [ready, autoplay, loop]);

    // Expose play/pause/seek API via ref
    useImperativeHandle(ref, () => ({
      play() {
        if (playingRef.current) return;
        playingRef.current = true;
        startTimeRef.current = null;
        rafRef.current = requestAnimationFrame(function tick(now: number) {
          if (!engineRef.current || !canvasRef.current) return;
          if (startTimeRef.current === null) startTimeRef.current = now;
          const engine = engineRef.current;
          const duration = engine.duration();
          let t = (now - startTimeRef.current) / 1000 + pausedAtRef.current;
          if (t >= duration) {
            t = duration;
            playingRef.current = false;
          }
          const ctx = canvasRef.current!.getContext('2d')!;
          const rgba = engine.render_frame(t);
          ctx.putImageData(new ImageData(new Uint8ClampedArray(rgba), engine.width(), engine.height()), 0, 0);
          if (playingRef.current) rafRef.current = requestAnimationFrame(tick);
        });
      },
      pause() {
        if (!playingRef.current) return;
        const engine = engineRef.current;
        if (engine && startTimeRef.current !== null) {
          pausedAtRef.current += (performance.now() - startTimeRef.current) / 1000;
        }
        playingRef.current = false;
        if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
      },
      seek(time: number) {
        pausedAtRef.current = Math.max(0, time);
        startTimeRef.current = null;
        const engine = engineRef.current;
        const canvas = canvasRef.current;
        if (!engine || !canvas) return;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;
        const rgba = engine.render_frame(pausedAtRef.current);
        ctx.putImageData(new ImageData(new Uint8ClampedArray(rgba), engine.width(), engine.height()), 0, 0);
      },
      getCurrentTime() {
        if (!playingRef.current || startTimeRef.current === null) return pausedAtRef.current;
        return (performance.now() - startTimeRef.current) / 1000 + pausedAtRef.current;
      },
    }));

    // Click → hit_test
    function handleClick(e: React.MouseEvent<HTMLCanvasElement>) {
      if (!onObjectClick || !engineRef.current || !canvasRef.current) return;
      const rect = canvasRef.current.getBoundingClientRect();
      const scaleX = engineRef.current.width() / rect.width;
      const scaleY = engineRef.current.height() / rect.height;
      const x = (e.clientX - rect.left) * scaleX;
      const y = (e.clientY - rect.top) * scaleY;
      const t = pausedAtRef.current + (playingRef.current && startTimeRef.current !== null
        ? (performance.now() - startTimeRef.current) / 1000
        : 0);
      const hit = engineRef.current.hit_test(x, y, t) ?? null;
      onObjectClick(hit);
    }

    const engine = engineRef.current;
    const canvasW = engine ? engine.width() : (scene.canvas.width);
    const canvasH = engine ? engine.height() : (scene.canvas.height);

    return (
      <canvas
        ref={canvasRef}
        width={canvasW}
        height={canvasH}
        className={className}
        style={{ display: 'block', width: displayWidth, height: displayHeight, ...style }}
        onClick={handleClick}
        aria-label={scene.meta?.title ?? 'Lumina animation'}
      />
    );
  }
);
