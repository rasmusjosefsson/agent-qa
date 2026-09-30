import React from 'react';
import {AbsoluteFill, interpolate, random, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {C, clamp, mono, sans} from '../theme';

const TESTS = [
  'login › redirects to dashboard',
  'signup › validates email',
  'checkout › applies coupon',
  'cart › persists after reload',
  'settings › saves profile',
  'search › shows results',
  'invite › sends email',
  'billing › updates card',
  'upload › accepts avatar',
  'logout › clears session',
];

const SELECTORS = [
  'div > div:nth-child(7) > span',
  '#btn-3f9a2c',
  '.css-1x9k2p > button',
  '//*[@id="root"]/div[2]/form/button',
  '[class*="Save_btn__"]',
  'button.sc-bdVaJa',
];

export const Problem: React.FC = () => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  const breakAt = 38;
  const shake = f > breakAt && f < breakAt + 26 ? (1 - (f - breakAt) / 26) * 18 : 0;
  const sx = (random(`x${f}`) - 0.5) * shake;
  const sy = (random(`y${f}`) - 0.5) * shake;
  const order = TESTS.map((_, i) => random(`o${i}`) * 30);
  const failing = order.filter((o) => f > breakAt + o).length;
  const passing = TESTS.length - failing;
  const head = spring({frame: f - 64, fps, config: {damping: 12}});
  return (
    <AbsoluteFill style={{transform: `translate(${sx}px, ${sy}px)`}}>
      <div style={{position: 'absolute', left: 120, top: 150, fontFamily: mono, fontSize: 24, color: C.dim, letterSpacing: 4}}>
        $ npm test <span style={{color: passing ? C.green : C.red, marginLeft: 30}}>{passing}/10 passing</span>
      </div>
      <div style={{position: 'absolute', left: 120, top: 210, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px 40px', width: 1680}}>
        {TESTS.map((t, i) => {
          const enter = spring({frame: f - i * 2, fps, config: {damping: 14}});
          const broken = f > breakAt + order[i];
          const pop = broken ? spring({frame: f - breakAt - order[i], fps, config: {damping: 8}}) : 0;
          const col = broken ? C.red : C.green;
          return (
            <div
              key={t}
              style={{
                height: 76,
                borderRadius: 14,
                background: broken ? `${C.red}14` : C.panel,
                border: `1px solid ${broken ? C.red + '88' : C.line}`,
                display: 'flex',
                alignItems: 'center',
                padding: '0 28px',
                gap: 22,
                fontFamily: mono,
                fontSize: 26,
                color: C.text,
                opacity: enter,
                transform: `translateX(${(1 - enter) * -60}px) rotate(${broken ? (random(`r${i}`) - 0.5) * 3 * pop : 0}deg)`,
                boxShadow: broken ? `0 0 30px ${C.red}33` : 'none',
              }}
            >
              <span style={{color: col, fontWeight: 700, fontSize: 30, transform: `scale(${broken ? 0.6 + pop * 0.4 : 1})`}}>{broken ? '✕' : '✓'}</span>
              <span style={{opacity: broken ? 0.6 : 1, textDecoration: broken ? 'line-through' : 'none'}}>{t}</span>
              <span style={{marginLeft: 'auto', fontSize: 18, color: broken ? C.red : C.dim}}>{broken ? 'TimeoutError' : `${Math.floor(random(`d${i}`) * 400 + 80)}ms`}</span>
            </div>
          );
        })}
      </div>
      {SELECTORS.map((s, i) => {
        const start = breakAt + 4 + i * 5;
        const t = f - start;
        if (t < 0) return null;
        const y = 560 + t * t * 0.3 - i * 30;
        const x = 160 + random(`sx${i}`) * 1300;
        const rot = (random(`sr${i}`) - 0.5) * 50 * (t / 40);
        return (
          <div
            key={s}
            style={{
              position: 'absolute',
              left: x,
              top: y,
              fontFamily: mono,
              fontSize: 30,
              color: C.red,
              background: '#1a0810',
              border: `1px dashed ${C.red}`,
              padding: '8px 16px',
              borderRadius: 8,
              transform: `rotate(${rot}deg)`,
              opacity: interpolate(t, [0, 4, 40], [0, 1, 0.9], clamp),
              boxShadow: `0 10px 40px ${C.red}44`,
            }}
          >
            {s}
          </div>
        );
      })}
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          bottom: 130,
          textAlign: 'center',
          fontFamily: sans,
          fontWeight: 900,
          fontSize: 96,
          letterSpacing: -3,
          color: C.text,
          opacity: head,
          transform: `scale(${0.8 + head * 0.2})`,
          textShadow: '0 10px 60px rgba(0,0,0,0.9)',
        }}
      >
        UI moves. <span style={{color: C.red}}>Selectors snap.</span>
      </div>
    </AbsoluteFill>
  );
};
