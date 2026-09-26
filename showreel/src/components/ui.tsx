import React from 'react';
import {Easing, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {C, clamp, mono, sans} from '../theme';

export const typed = (text: string, f: number, start: number, cps = 1.2) =>
  text.slice(0, Math.max(0, Math.floor((f - start) * cps)));

type Key = [number, number, number];
export const path = (f: number, keys: Key[]): {x: number; y: number} => {
  if (f <= keys[0][0]) return {x: keys[0][1], y: keys[0][2]};
  for (let i = 0; i < keys.length - 1; i++) {
    const [f0, x0, y0] = keys[i];
    const [f1, x1, y1] = keys[i + 1];
    if (f <= f1) {
      const t = interpolate(f, [f0, f1], [0, 1], {...clamp, easing: Easing.inOut(Easing.cubic)});
      return {x: x0 + (x1 - x0) * t, y: y0 + (y1 - y0) * t};
    }
  }
  const last = keys[keys.length - 1];
  return {x: last[1], y: last[2]};
};

export const useEnter = (delay: number, damping = 14) => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  return spring({frame: f - delay, fps, config: {damping, mass: 0.7}});
};

export const SceneTag: React.FC<{num: string; label: string; color: string; title: string}> = ({
  num,
  label,
  color,
  title,
}) => {
  const f = useCurrentFrame();
  const e = useEnter(0);
  const line = interpolate(f, [4, 24], [0, 120], {...clamp, easing: Easing.out(Easing.cubic)});
  const words = title.split(' ');
  return (
    <div style={{position: 'absolute', left: 120, top: 110}}>
      <div style={{display: 'flex', alignItems: 'center', gap: 18, fontFamily: mono, fontSize: 22, letterSpacing: 6, color, opacity: e}}>
        <span style={{fontWeight: 700}}>{num}</span>
        <div style={{width: line, height: 2, background: color, boxShadow: `0 0 10px ${color}`}} />
        <span>{label}</span>
      </div>
      <div style={{fontFamily: sans, fontWeight: 900, fontSize: 64, color: C.text, marginTop: 10, letterSpacing: -2, display: 'flex', gap: 18}}>
        {words.map((w, i) => {
          const s = spring({frame: f - 6 - i * 3, fps: 30, config: {damping: 13, mass: 0.6}});
          return (
            <span key={i} style={{display: 'inline-block', overflow: 'hidden'}}>
              <span style={{display: 'inline-block', transform: `translateY(${(1 - s) * 80}px)`, opacity: s}}>{w}</span>
            </span>
          );
        })}
      </div>
    </div>
  );
};

export const BrowserWindow: React.FC<{
  url: string;
  width: number;
  height: number;
  children?: React.ReactNode;
  accent?: string;
  style?: React.CSSProperties;
}> = ({url, width, height, children, accent = C.cyan, style}) => (
  <div
    style={{
      position: 'absolute',
      width,
      height,
      borderRadius: 22,
      background: '#0b0e1a',
      border: `1px solid ${C.line}`,
      boxShadow: `0 40px 120px rgba(0,0,0,0.6), 0 0 0 1px ${accent}22, 0 0 80px ${accent}18`,
      overflow: 'hidden',
      ...style,
    }}
  >
    <div style={{height: 56, display: 'flex', alignItems: 'center', gap: 10, padding: '0 22px', background: '#111527', borderBottom: `1px solid ${C.line}`}}>
      {['#ff5f57', '#febc2e', '#28c840'].map((c) => (
        <div key={c} style={{width: 14, height: 14, borderRadius: 7, background: c}} />
      ))}
      <div style={{marginLeft: 24, flex: 1, height: 34, borderRadius: 17, background: '#070913', display: 'flex', alignItems: 'center', padding: '0 18px', fontFamily: mono, fontSize: 18, color: C.dim}}>
        <span style={{color: C.green}}>https://</span>
        {url}
      </div>
    </div>
    <div style={{position: 'relative', width, height: height - 56}}>{children}</div>
  </div>
);

export const Cursor: React.FC<{x: number; y: number; clicks?: number[]; color?: string}> = ({x, y, clicks = [], color = C.cyan}) => {
  const f = useCurrentFrame();
  const pressed = clicks.some((c) => f >= c && f < c + 5);
  return (
    <>
      {clicks.map((c) => {
        const t = interpolate(f, [c, c + 18], [0, 1], clamp);
        if (f < c || t >= 1) return null;
        return (
          <div
            key={c}
            style={{
              position: 'absolute',
              left: x - 40,
              top: y - 40,
              width: 80,
              height: 80,
              borderRadius: 40,
              border: `3px solid ${color}`,
              transform: `scale(${0.2 + t * 1.3})`,
              opacity: 1 - t,
            }}
          />
        );
      })}
      <svg
        width="34"
        height="40"
        viewBox="0 0 17 20"
        style={{position: 'absolute', left: x - 3, top: y - 2, transform: `scale(${pressed ? 0.82 : 1})`, filter: 'drop-shadow(0 4px 8px rgba(0,0,0,0.6))'}}
      >
        <path d="M1 1 L1 16 L5 12 L8 19 L11 18 L8 11 L14 11 Z" fill="#fff" stroke="#000" strokeWidth="1.2" strokeLinejoin="round" />
      </svg>
    </>
  );
};

export const Verb: React.FC<{verb: string; color: string}> = ({verb, color}) => (
  <span
    style={{
      fontFamily: mono,
      fontWeight: 700,
      fontSize: 18,
      color,
      background: `${color}1f`,
      border: `1px solid ${color}66`,
      padding: '4px 12px',
      borderRadius: 8,
      letterSpacing: 1,
    }}
  >
    {verb}
  </span>
);
