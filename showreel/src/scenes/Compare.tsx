import React from 'react';
import {AbsoluteFill, Easing, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {C, clamp, mono, sans} from '../theme';
import {SceneTag} from '../components/ui';

type Row = {id: string; label: string; a: string; b: string; ta: number; tb: number; diff?: boolean};
const ROWS: Row[] = [
  {id: 's1', label: 'goto /signup', a: 'ok', b: 'ok', ta: 0.42, tb: 0.4},
  {id: 's2', label: 'fill Email', a: 'ok', b: 'ok', ta: 0.2, tb: 0.22},
  {id: 's3', label: 'click "Save"', a: 'ok', b: 'healed', ta: 0.3, tb: 0.55},
  {id: 's4', label: 'check toast', a: '"Saved"', b: '"Saved!"', ta: 0.25, tb: 0.26, diff: true},
  {id: 's5', label: 'check url', a: 'ok', b: 'ok', ta: 0.12, tb: 0.12},
];

const Panel: React.FC<{title: string; side: 'a' | 'b'; left: number; delay: number}> = ({title, side, left, delay}) => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  const e = spring({frame: f - delay, fps, config: {damping: 14}});
  return (
    <div style={{position: 'absolute', left, top: 300, width: 780, opacity: e, transform: `translateY(${(1 - e) * 60}px)`}}>
      <div style={{fontFamily: mono, fontSize: 22, color: C.dim, letterSpacing: 3, marginBottom: 18}}>{title}</div>
      {ROWS.map((r, i) => {
        const re = spring({frame: f - delay - 6 - i * 4, fps, config: {damping: 14}});
        const v = side === 'a' ? r.a : r.b;
        const t = side === 'a' ? r.ta : r.tb;
        const col = r.diff ? C.amber : v === 'healed' ? C.violet : C.green;
        const bar = interpolate(f, [delay + 14 + i * 4, delay + 40 + i * 4], [0, t], {...clamp, easing: Easing.out(Easing.cubic)});
        return (
          <div
            key={r.id}
            style={{
              height: 78,
              marginBottom: 12,
              borderRadius: 14,
              background: r.diff && f > 50 ? `${C.amber}14` : C.panel,
              border: `1px solid ${r.diff && f > 50 ? C.amber : C.line}`,
              display: 'flex',
              alignItems: 'center',
              padding: '0 24px',
              gap: 20,
              fontFamily: mono,
              fontSize: 22,
              color: C.text,
              opacity: re,
            }}
          >
            <span style={{color: C.dim, width: 36}}>{r.id}</span>
            <span style={{width: 230}}>{r.label}</span>
            <div style={{width: 180, height: 8, borderRadius: 4, background: C.line}}>
              <div style={{width: `${bar * 100}%`, height: '100%', borderRadius: 4, background: col}} />
            </div>
            <span style={{marginLeft: 'auto', color: col, fontWeight: 700}}>{v}</span>
          </div>
        );
      })}
    </div>
  );
};

export const Compare: React.FC = () => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  const zoom = interpolate(f, [55, 110], [1, 1.12], {...clamp, easing: Easing.inOut(Easing.cubic)});
  const delta = spring({frame: f - 58, fps, config: {damping: 9}});
  const scan = interpolate(f, [30, 60], [290, 780], clamp);
  return (
    <AbsoluteFill>
      <div style={{position: 'absolute', inset: 0, transform: `scale(${zoom})`, transformOrigin: '50% 72%'}}>
        <SceneTag num="04" label="COMPARE" color={C.cyan} title="See exactly what changed." />
        <Panel title="run a1f3 · main" side="a" left={130} delay={4} />
        <Panel title="run c7d9 · feat/checkout" side="b" left={1010} delay={12} />
        <div
          style={{
            position: 'absolute',
            left: 110,
            right: 110,
            top: scan,
            height: 3,
            background: C.cyan,
            boxShadow: `0 0 30px ${C.cyan}`,
            opacity: interpolate(f, [28, 32, 58, 62], [0, 1, 1, 0], clamp),
          }}
        />
        <div
          style={{
            position: 'absolute',
            left: 830,
            top: 590,
            width: 260,
            height: 110,
            borderRadius: 55,
            background: C.amber,
            color: '#161000',
            fontFamily: sans,
            fontWeight: 900,
            fontSize: 44,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            transform: `scale(${delta})`,
            boxShadow: `0 0 70px ${C.amber}`,
          }}
        >
          Δ 1 diff
        </div>
      </div>
    </AbsoluteFill>
  );
};
