import React from 'react';
import {AbsoluteFill, Easing, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {C, clamp, mono, sans} from '../theme';
import {BrowserWindow, Cursor, SceneTag, path} from '../components/ui';

const RUNGS: [string, string, boolean, number][] = [
  ['css', '#save-btn', false, 50],
  ['testid', 'save', false, 62],
  ['role', 'button "Save…"', true, 76],
];

export const Heal: React.FC = () => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  const deploy = interpolate(f, [18, 34], [0, 1], {...clamp, easing: Easing.inOut(Easing.cubic)});
  const bx = 150 + deploy * 330;
  const by = 190 + deploy * 150;
  const bw = 260 + deploy * 60;
  const cur = path(f, [
    [0, 700, 520],
    [26, 700, 520],
    [42, 280, 225],
    [82, 280, 225],
    [96, 640, 372],
  ]);
  const missT = interpolate(f, [43, 48, 80], [0, 1, 0.6], clamp);
  const stamp = spring({frame: f - 100, fps, config: {damping: 9}});
  const tagDeploy = interpolate(f, [16, 20, 40, 46], [0, 1, 1, 0], clamp);
  const climb = interpolate(f, [50, 62, 76], [0, 1, 2], {...clamp, easing: Easing.inOut(Easing.cubic)});
  const healed = f >= 97;
  return (
    <AbsoluteFill>
      <SceneTag num="03" label="AUTO-HEAL" color={C.violet} title="UI changed? It adapts." />
      <BrowserWindow url="app.example.com/settings" width={900} height={600} accent={C.violet} style={{left: 120, top: 320}}>
        <div style={{position: 'absolute', left: 40, top: 40, fontFamily: sans, fontWeight: 800, fontSize: 34, color: C.text}}>Profile</div>
        <div style={{position: 'absolute', left: 40, top: 100, width: 520, height: 18, borderRadius: 6, background: '#1c2240'}} />
        <div style={{position: 'absolute', left: 40, top: 134, width: 380, height: 18, borderRadius: 6, background: '#1c2240'}} />
        <div
          style={{
            position: 'absolute',
            left: bx,
            top: by,
            width: bw,
            height: 64,
            borderRadius: 14 + deploy * 20,
            background: healed ? C.green : `linear-gradient(90deg, ${C.cyan}, ${C.violet})`,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontFamily: sans,
            fontWeight: 800,
            fontSize: 24,
            color: '#05060b',
            boxShadow: healed ? `0 0 50px ${C.green}` : 'none',
          }}
        >
          {deploy < 0.5 ? 'Save' : 'Save changes'}
        </div>
        <div
          style={{
            position: 'absolute',
            left: 200,
            top: 196,
            width: 160,
            height: 56,
            border: `3px dashed ${C.red}`,
            borderRadius: 14,
            opacity: missT,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            color: C.red,
            fontFamily: mono,
            fontWeight: 700,
            fontSize: 20,
          }}
        >
          MISS
        </div>
        <div style={{position: 'absolute', right: 30, top: 30, fontFamily: mono, fontSize: 18, color: C.amber, border: `1px solid ${C.amber}`, borderRadius: 8, padding: '6px 12px', opacity: tagDeploy}}>
          ⚡ v2 deployed
        </div>
        <Cursor x={cur.x} y={cur.y} clicks={[43, 97]} color={f > 90 ? C.green : C.red} />
      </BrowserWindow>
      <div style={{position: 'absolute', left: 1110, top: 320, width: 690}}>
        <div style={{fontFamily: mono, fontSize: 20, color: C.dim, letterSpacing: 3, marginBottom: 22}}>FALLBACK LADDER</div>
        <div style={{position: 'relative'}}>
          <div
            style={{
              position: 'absolute',
              left: -26,
              top: 38 + climb * 116,
              width: 8,
              height: 20,
              borderRadius: 4,
              background: C.violet,
              boxShadow: `0 0 20px ${C.violet}`,
              opacity: interpolate(f, [48, 52], [0, 1], clamp),
            }}
          />
          {RUNGS.map(([kind, sel, ok, at]) => {
            const e = spring({frame: f - at, fps, config: {damping: 12}});
            const res = spring({frame: f - at - 6, fps, config: {damping: 8}});
            const col = ok ? C.green : C.red;
            return (
              <div
                key={kind}
                style={{
                  height: 96,
                  marginBottom: 20,
                  borderRadius: 16,
                  background: C.panel,
                  border: `1px solid ${res > 0.5 ? col + '99' : C.line}`,
                  boxShadow: ok && res > 0.5 ? `0 0 40px ${C.green}55` : 'none',
                  display: 'flex',
                  alignItems: 'center',
                  padding: '0 28px',
                  gap: 20,
                  fontFamily: mono,
                  fontSize: 26,
                  color: C.text,
                  opacity: e,
                  transform: `translateX(${(1 - e) * 80}px)`,
                }}
              >
                <span style={{color: C.dim, width: 90}}>{kind}</span>
                <span>{sel}</span>
                <span style={{marginLeft: 'auto', fontSize: 38, fontWeight: 700, color: col, transform: `scale(${res})`}}>{ok ? '✓' : '✕'}</span>
              </div>
            );
          })}
        </div>
        <div
          style={{
            marginTop: 18,
            display: 'inline-block',
            fontFamily: sans,
            fontWeight: 900,
            fontSize: 58,
            color: C.green,
            border: `5px solid ${C.green}`,
            borderRadius: 16,
            padding: '6px 26px',
            transform: `rotate(-6deg) scale(${1.6 - stamp * 0.6})`,
            opacity: stamp,
            textShadow: `0 0 30px ${C.green}`,
          }}
        >
          HEALED
        </div>
        <div style={{display: 'inline-block', marginLeft: 30, fontFamily: mono, fontSize: 20, color: C.dim, opacity: interpolate(f, [110, 122], [0, 1], clamp)}}>
          heal-promote → scenario.json
        </div>
      </div>
    </AbsoluteFill>
  );
};
