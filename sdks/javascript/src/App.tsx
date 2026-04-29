import React from 'react';
import { LuminaPlayer } from './LuminaPlayer';

// Import the mascot scene JSON. 
// Depending on your bundler (Vite/Webpack), you might need a JSON loader.
import mascotScene from '../assets/mascot.lsf';

const App: React.FC = () => {
  return (
    <div style={{ 
      display: 'flex', 
      flexDirection: 'column', 
      alignItems: 'center', 
      justifyContent: 'center', 
      height: '100vh',
      backgroundColor: '#1a1a1a' 
    }}>
      <h1 style={{ color: '#fff', marginBottom: '20px' }}>Lumina Interactive Mascot</h1>
      <div style={{ 
        border: '4px solid #333', 
        borderRadius: '8px', 
        overflow: 'hidden' 
      }}>
        <LuminaPlayer 
          scene={mascotScene} 
          width={600} 
          height={400} 
        />
      </div>
      <p style={{ color: '#888', marginTop: '20px' }}>
        Move your mouse over the canvas to track the mascot's eyes.
      </p>
    </div>
  );
};

export default App;
