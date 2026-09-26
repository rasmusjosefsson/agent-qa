import React from 'react';
import {AbsoluteFill, useCurrentFrame} from 'remotion';
import {C} from '../theme';

export const Background: React.FC = () => {
  const f = useCurrentFrame();
  return (
    <AbsoluteFill style={{background: C.bg, overflow: 'hidden'}}>
      <div
        style={{
          position: 'absolute',
          width: 1100,
          height: 1100,
          borderRadius: '50%',
          background: `radial-gradient(circle, ${C.violet}44, transparent 62%)`,
          left: 150 + Math.sin(f / 70) * 260,
          top: -380 + Math.cos(f / 90) * 120,
        }}
      />
      <div
        style={{
          position: 'absolute',
          width: 1000,
          height: 1000,
          borderRadius: '50%',
          background: `radial-gradient(circle, ${C.cyan}30, transparent 62%)`,
          right: -200 + Math.cos(f / 60) * 220,
          bottom: -420 + Math.sin(f / 75) * 140,
        }}
      />
      <div
        style={{
          position: 'absolute',
          left: -400,
          right: -400,
          top: '52%',
          height: 1200,
          backgroundImage: `linear-gradient(${C.line} 2px, transparent 2px), linear-gradient(90deg, ${C.line} 2px, transparent 2px)`,
          backgroundSize: '90px 90px',
          backgroundPosition: `0 ${(f * 3) % 90}px`,
          transform: 'perspective(900px) rotateX(62deg)',
          transformOrigin: '50% 0%',
          maskImage: 'linear-gradient(to bottom, transparent, black 25%, black 60%, transparent)',
          WebkitMaskImage:
            'linear-gradient(to bottom, transparent, black 25%, black 60%, transparent)',
        }}
      />
      <svg width="100%" height="100%" style={{position: 'absolute', opacity: 0.07, mixBlendMode: 'overlay'}}>
        <filter id="grain">
          <feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2" seed={f % 12} />
        </filter>
        <rect width="100%" height="100%" filter="url(#grain)" />
      </svg>
      <AbsoluteFill
        style={{background: 'radial-gradient(ellipse at center, transparent 45%, rgba(0,0,0,0.75) 100%)'}}
      />
    </AbsoluteFill>
  );
};
