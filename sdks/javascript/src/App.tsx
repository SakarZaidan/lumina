import React from 'react';
import { LuminaPlayer } from './LuminaPlayer';
import type { LuminaScene } from './types';

// Minimal demo scene — replace with your own or fetch from the /render API.
const demoScene: LuminaScene = {
  version: '1.0',
  meta: { title: 'SDK Demo', author: 'Lumina', created_at: '2026-05-20' },
  canvas: { width: 640, height: 360, fps: 30, duration: 3.0, background: '#0F0F1A' },
  objects: {
    circle: {
      type: 'Circle',
      properties: { cx: 80, cy: 180, radius: 40, fill: '#E040FB', opacity: 0, z_index: 1 },
    },
    label: {
      type: 'Text',
      properties: { content: 'Lumina SDK', x: 160, y: 210, font_size: 48, color: '#FFFFFF', opacity: 0, z_index: 2 },
    },
  },
  timeline: [
    { time: 0.0, object: 'circle', state: { opacity: 0.0 }, easing: 'linear' },
    { time: 1.0, object: 'circle', state: { opacity: 1.0 }, easing: 'ease_out_cubic' },
    { time: 0.5, object: 'label', state: { opacity: 0.0 }, easing: 'linear' },
    { time: 1.5, object: 'label', state: { opacity: 1.0 }, easing: 'ease_out_sine' },
  ],
  events: [],
};

const App: React.FC = () => {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', padding: 32, background: '#111', minHeight: '100vh' }}>
      <h1 style={{ color: '#E040FB', fontFamily: 'sans-serif', marginBottom: 24 }}>Lumina SDK Demo</h1>
      <LuminaPlayer
        scene={demoScene}
        autoplay
        loop
        displayWidth={640}
        displayHeight={360}
        onObjectClick={(id) => console.log('clicked:', id)}
        style={{ border: '2px solid #333', borderRadius: 8 }}
      />
      <p style={{ color: '#888', marginTop: 16, fontFamily: 'monospace', fontSize: 13 }}>
        Click any object — the ID is logged to the console.
      </p>
    </div>
  );
};

export default App;
