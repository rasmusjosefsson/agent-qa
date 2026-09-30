import React from 'react';
import {AbsoluteFill, interpolate, random, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {C, clamp, mono, sans} from '../theme';
import {typed} from '../components/ui';

export const Hook: React.FC = () => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  const s1 = spring({frame: f, fps, config: {damping: 12, mass: 0.6}});
  const s2 = spring({frame: f - 12, fps, config: {damping: 9, mass: 0.7}});
  const glitchOn = f > 16 && f < 44 && random(`on${Math.floor(f / 3)}`) > 0.35;
  const g = glitchOn ? (random(`g${f}`) - 0.5) * 36 : 0;
  const slice = glitchOn ? random(`s${f}`) * 100 : -1;
  const push = interpolate(f, [0, 90], [1, 1.1]);
  const flash = interpolate(f, [12, 14, 22], [0, 0.35, 0], clamp);
  const q = typed('Did your tests notice?', f, 44, 1.3);
  const caret = Math.floor(f / 8) % 2 === 0;
  return (
    <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center', transform: `scale(${push})`}}>
      <div style={{fontFamily: sans, fontWeight: 900, fontSize: 210, letterSpacing: -8, lineHeight: 0.92, textAlign: 'center'}}>
        <div style={{color: C.text, transform: `translateY(${(1 - s1) * 120}px)`, opacity: s1, filter: `blur(${(1 - s1) * 20}px)`}}>
          Your UI
        </div>
        <div
          style={{
            position: 'relative',
            transform: `scale(${0.6 + s2 * 0.4}) translateX(${g}px)`,
            opacity: Math.min(1, s2 * 2),
            background: `linear-gradient(90deg, ${C.red}, ${C.amber})`,
            WebkitBackgroundClip: 'text',
            color: 'transparent',
            textShadow: glitchOn ? `${g * 0.6}px 0 ${C.cyan}88, ${-g * 0.6}px 0 ${C.red}88` : 'none',
            clipPath: slice >= 0 ? `polygon(0 0, 100% 0, 100% ${slice}%, 0 ${slice + 12}%, 0 100%, 100% 100%, 100% ${slice + 14}%, 0 ${slice + 2}%)` : undefined,
          }}
        >
          changed.
        </div>
      </div>
      <div style={{position: 'absolute', top: 820, fontFamily: mono, fontSize: 52, color: C.dim}}>
        {q}
        <span style={{color: C.cyan, opacity: f > 40 && caret ? 1 : 0}}>▍</span>
      </div>
      <AbsoluteFill style={{background: C.text, opacity: flash, mixBlendMode: 'overlay'}} />
    </AbsoluteFill>
  );
};
