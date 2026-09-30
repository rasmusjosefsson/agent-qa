import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {C, clamp, mono, sans} from '../theme';
import {SceneTag, Verb} from '../components/ui';

const NODES: [string, string][] = [
  ['goto', C.cyan],
  ['fill', C.violet],
  ['click', C.green],
  ['check', C.amber],
];
const RUNS: [number, number, string][] = [
  [18, 62, '7f3a'],
  [64, 90, '91cc'],
  [92, 108, 'e02b'],
];
const X0 = 260;
const X1 = 1660;

export const Replay: React.FC = () => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  let run = 0;
  let p = 0;
  RUNS.forEach(([a, b], i) => {
    if (f >= a) {
      run = i;
      p = interpolate(f, [a, b], [0, 1], clamp);
    }
  });
  const started = f >= RUNS[0][0];
  const headX = X0 + (X1 - X0) * p;
  const lineIn = interpolate(f, [0, 18], [0, 1], clamp);
  const tail = spring({frame: f - 100, fps, config: {damping: 12}});
  return (
    <AbsoluteFill>
      <SceneTag num="02" label="REPLAY" color={C.green} title="Run it forever." />
      <div style={{position: 'absolute', right: 130, top: 130, textAlign: 'right', fontFamily: mono}}>
        <div style={{fontSize: 20, color: C.dim, letterSpacing: 4}}>RUN</div>
        <div style={{fontSize: 110, fontWeight: 700, color: C.green, lineHeight: 1, textShadow: `0 0 40px ${C.green}88`}}>#{started ? run + 1 : 0}</div>
      </div>
      <div style={{position: 'absolute', left: X0, top: 598, width: (X1 - X0) * lineIn, height: 4, background: C.line}} />
      {started && (
        <div style={{position: 'absolute', left: X0, top: 596, width: headX - X0, height: 8, borderRadius: 4, background: `linear-gradient(90deg, transparent, ${C.green})`, boxShadow: `0 0 24px ${C.green}`}} />
      )}
      {NODES.map(([verb, col], i) => {
        const x = X0 + ((X1 - X0) * i) / (NODES.length - 1);
        const hit = started && headX >= x - 1;
        const hitT = hit ? Math.max(0, f - (RUNS[run][0] + ((RUNS[run][1] - RUNS[run][0]) * i) / (NODES.length - 1))) : 99;
        const pop = spring({frame: hitT, fps, config: {damping: 8}});
        const e = spring({frame: f - 4 - i * 3, fps, config: {damping: 12}});
        const thumb = spring({frame: f - 22 - i * 13, fps, config: {damping: 13}});
        return (
          <React.Fragment key={verb}>
            <div
              style={{
                position: 'absolute',
                left: x - 190 / 2,
                top: 300,
                width: 190,
                height: 200,
                borderRadius: 14,
                background: C.panel,
                border: `1px solid ${C.line}`,
                overflow: 'hidden',
                opacity: thumb,
                transform: `translateY(${(1 - thumb) * 40}px)`,
              }}
            >
              <div style={{height: 26, background: '#111527', display: 'flex', gap: 6, alignItems: 'center', padding: '0 10px'}}>
                {[0, 1, 2].map((k) => <div key={k} style={{width: 8, height: 8, borderRadius: 4, background: C.dim}} />)}
              </div>
              <div style={{margin: '16px 20px', height: 16, width: 110, borderRadius: 4, background: '#2a3050'}} />
              <div style={{margin: '0 20px 10px', height: 22, borderRadius: 5, border: `1px solid ${i === 1 ? C.cyan : '#2a3050'}`}} />
              <div style={{margin: '0 20px', height: 24, borderRadius: 5, background: i >= 2 ? `linear-gradient(90deg, ${C.cyan}, ${C.violet})` : '#2a3050'}} />
              <div style={{position: 'absolute', bottom: 10, left: 0, right: 0, textAlign: 'center', fontFamily: mono, fontSize: 13, color: C.dim}}>audit · dom · png</div>
            </div>
            <div
              style={{
                position: 'absolute',
                left: x - 44,
                top: 556,
                width: 88,
                height: 88,
                borderRadius: 44,
                background: hit ? C.green : C.panel,
                border: `3px solid ${hit ? C.green : col}`,
                boxShadow: hit ? `0 0 ${30 + 30 * (1 - pop)}px ${C.green}` : 'none',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontSize: 44,
                fontWeight: 900,
                color: '#04130b',
                transform: `scale(${e * (hit ? 0.8 + pop * 0.2 + (1 - pop) * 0.3 : 1)})`,
              }}
            >
              {hit ? '✓' : ''}
            </div>
            <div style={{position: 'absolute', left: x - 100, width: 200, top: 670, display: 'flex', justifyContent: 'center', opacity: e}}>
              <Verb verb={verb} color={col} />
            </div>
            {verb === 'fill' && (
              <div style={{position: 'absolute', left: x - 200, width: 400, top: 720, textAlign: 'center', fontFamily: mono, fontSize: 22, color: C.amber, opacity: e}}>
                qa-{started ? RUNS[run][2] : '····'}@example.com
              </div>
            )}
          </React.Fragment>
        );
      })}
      <div style={{position: 'absolute', left: 0, right: 0, top: 820, textAlign: 'center', fontFamily: sans, fontWeight: 800, fontSize: 52, color: C.text, opacity: tail, transform: `translateY(${(1 - tail) * 30}px)`}}>
        Same scenario. <span style={{color: C.amber}}>Fresh data</span> every run.
      </div>
    </AbsoluteFill>
  );
};
