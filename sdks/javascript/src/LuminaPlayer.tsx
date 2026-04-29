import React, { useEffect, useRef, useState } from 'react';
import init, { LuminaEngine } from '../wasm/lumina_wasm';

interface LuminaPlayerProps {
  scene: any;
  width?: number;
  height?: number;
}

export const LuminaPlayer: React.FC<LuminaPlayerProps> = ({ scene, width, height }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const engineRef = useRef<LuminaEngine | null>(null);
  const [time, setTime] = useState(0);

  useEffect(() => {
    const initEngine = async () => {
      await init();
      engineRef.current = new LuminaEngine(scene);
    };
    initEngine();
  }, [scene]);

  // High-frequency tracking loop (120Hz potential)
  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!engineRef.current || !canvasRef.current) return;
    const rect = canvasRef.current.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    // Direct event piping to the engine for sub-millisecond processing
    engineRef.current.process_event({
      object_id: "head",
      trigger: "mouse_move",
      payload: { x, y }
    });
  };

  const handlePasswordFocus = () => {
    engineRef.current?.process_event({
      object_id: "password_field",
      trigger: "focus",
      payload: null
    });
  };

  const triggerSuccess = () => {
    engineRef.current?.process_event({
      object_id: "login_button",
      trigger: "click",
      payload: { status: "success" }
    });
  };

  useEffect(() => {
    const render = () => {
      if (engineRef.current && canvasRef.current) {
        const ctx = canvasRef.current.getContext('2d');
        if (ctx) {
          const frameData = engineRef.current.render_frame(time);
          const imgData = new ImageData(new Uint8ClampedArray(frameData), 600, 400);
          ctx.putImageData(imgData, 0, 0);
        }
      }
      requestAnimationFrame(render);
    };
    requestAnimationFrame(render);
  }, [time]);

  return (
    <canvas 
      ref={canvasRef} 
      width={width || 600} 
      height={height || 400}
      onMouseMove={handleMouseMove}
      onFocus={handlePasswordFocus}
    />
  );
};
