import React from 'react';
import {AbsoluteFill, interpolate, useCurrentFrame, useVideoConfig} from 'remotion';
import {C, clamp, mono} from '../theme';

const Corner: React.FC<{style: React.CSSProperties}> = ({style}) => (
  <div style={{position: 'absolute', width: 36, height: 36, ...style}} />
);

export const Hud: React.FC = () => {
  const f = useCurrentFrame();
  const {durationInFrames, fps} = useVideoConfig();
  const secs = Math.floor(f / fps);
  const tc = `00:00:${String(secs).padStart(2, '0')}:${String(f % fps).padStart(2, '0')}`;
  const intro = interpolate(f, [0, 20], [0, 1], clamp);
  const b = `2px solid ${C.text}55`;
  return (
    <AbsoluteFill style={{fontFamily: mono, color: C.dim, fontSize: 18, letterSpacing: 3, opacity: intro}}>
      <Corner style={{left: 40, top: 40, borderLeft: b, borderTop: b}} />
      <Corner style={{right: 40, top: 40, borderRight: b, borderTop: b}} />
      <Corner style={{left: 40, bottom: 40, borderLeft: b, borderBottom: b}} />
      <Corner style={{right: 40, bottom: 40, borderRight: b, borderBottom: b}} />
      <div style={{position: 'absolute', left: 92, top: 50}}>
        <span style={{color: C.red}}>●</span> AGENT-QA / SHOWREEL
      </div>
      <div style={{position: 'absolute', right: 92, top: 50}}>{tc}</div>
      <div style={{position: 'absolute', left: 92, right: 92, bottom: 58, height: 3, background: C.line}}>
        <div
          style={{
            width: `${(f / (durationInFrames - 1)) * 100}%`,
            height: '100%',
            background: `linear-gradient(90deg, ${C.cyan}, ${C.violet}, ${C.green})`,
            boxShadow: `0 0 12px ${C.cyan}`,
          }}
        />
      </div>
    </AbsoluteFill>
  );
};
