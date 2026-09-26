import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {C, clamp, mono, sans} from '../theme';
import {BrowserWindow, Cursor, SceneTag, Verb, path, typed} from '../components/ui';

type Step = {at: number; verb: string; color: string; body: React.ReactNode};

const STEPS: Step[] = [
  {at: 22, verb: 'goto', color: C.cyan, body: <>https://app.example.com/signup</>},
  {at: 48, verb: 'fill', color: C.violet, body: <>Email ← qa-<span style={{color: C.amber}}>{'{{vars._unique}}'}</span>@example.com</>},
  {at: 108, verb: 'click', color: C.green, body: <>role=button "Save"</>},
  {at: 130, verb: 'check', color: C.amber, body: <>text "Saved" exists</>},
];

export const Record: React.FC = () => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  const win = spring({frame: f - 2, fps, config: {damping: 15}});
  const cur = path(f, [
    [0, 860, 560],
    [22, 860, 560],
    [42, 470, 232],
    [88, 470, 232],
    [104, 530, 414],
  ]);
  const email = typed('qa-7f3a@example.com', f, 52, 0.6);
  const pressed = f >= 105 && f < 112;
  const toast = spring({frame: f - 114, fps, config: {damping: 12}});
  const rec = Math.floor(f / 12) % 2 === 0;
  const url = typed('app.example.com/signup', f, 6, 1.4);
  const field = (focused: boolean): React.CSSProperties => ({
    position: 'absolute',
    left: 290,
    width: 480,
    height: 58,
    borderRadius: 12,
    background: '#070913',
    border: `2px solid ${focused ? C.cyan : C.line}`,
    boxShadow: focused ? `0 0 0 5px ${C.cyan}22` : 'none',
    display: 'flex',
    alignItems: 'center',
    padding: '0 18px',
    fontFamily: mono,
    fontSize: 22,
    color: C.text,
  });
  return (
    <AbsoluteFill>
      <SceneTag num="01" label="RECORD" color={C.red} title="Just use your app." />
      <div style={{position: 'absolute', left: 0, top: 0, width: 1920, height: 1080, perspective: 2000}}>
        <BrowserWindow
          url={url}
          width={1060}
          height={680}
          accent={C.red}
          style={{left: 120, top: 290, opacity: win, transform: `translateY(${(1 - win) * 100}px) rotateY(${(1 - win) * 18 + 6}deg)`, transformOrigin: '0% 50%'}}
        >
          <div style={{position: 'absolute', left: 24, top: 20, fontFamily: mono, fontSize: 18, color: C.red, letterSpacing: 3, opacity: rec ? 1 : 0.35}}>● REC</div>
          <div style={{position: 'absolute', left: 240, top: 50, width: 580, height: 520, borderRadius: 20, background: C.panel, border: `1px solid ${C.line}`}} />
          <div style={{position: 'absolute', left: 290, top: 90, fontFamily: sans, fontWeight: 800, fontSize: 40, color: C.text}}>Create account</div>
          <div style={{position: 'absolute', left: 290, top: 170, fontFamily: sans, fontSize: 18, color: C.dim}}>Email</div>
          <div style={{...field(f >= 45 && f < 100), top: 200}}>
            {email}
            {f >= 45 && f < 100 && Math.floor(f / 8) % 2 === 0 ? <span style={{color: C.cyan}}>▍</span> : null}
          </div>
          <div style={{position: 'absolute', left: 290, top: 280, fontFamily: sans, fontSize: 18, color: C.dim}}>Password</div>
          <div style={{...field(false), top: 310}}>{f > 60 ? '••••••••••' : ''}</div>
          <div
            style={{
              position: 'absolute',
              left: 290,
              top: 384,
              width: 480,
              height: 62,
              borderRadius: 12,
              background: `linear-gradient(90deg, ${C.cyan}, ${C.violet})`,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontFamily: sans,
              fontWeight: 800,
              fontSize: 24,
              color: '#05060b',
              transform: `scale(${pressed ? 0.96 : 1})`,
            }}
          >
            Save
          </div>
          <div
            style={{
              position: 'absolute',
              left: 400,
              top: 480,
              width: 260,
              height: 54,
              borderRadius: 27,
              background: `${C.green}22`,
              border: `1px solid ${C.green}`,
              color: C.green,
              fontFamily: sans,
              fontWeight: 800,
              fontSize: 22,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              opacity: toast,
              transform: `translateY(${(1 - toast) * 20}px)`,
            }}
          >
            ✓ Saved
          </div>
          <Cursor x={cur.x} y={cur.y} clicks={[44, 105]} color={C.red} />
        </BrowserWindow>
      </div>
      <div style={{position: 'absolute', left: 1250, top: 290, width: 560}}>
        <div style={{fontFamily: mono, fontSize: 20, color: C.dim, letterSpacing: 3, marginBottom: 20}}>scenario.json</div>
        {STEPS.map((s, i) => {
          const e = spring({frame: f - s.at, fps, config: {damping: 13}});
          const glow = interpolate(f - s.at, [0, 6, 24], [0, 1, 0], clamp);
          return (
            <div
              key={i}
              style={{
                marginBottom: 18,
                padding: '20px 22px',
                borderRadius: 16,
                background: C.panel,
                border: `1px solid ${s.color}${glow > 0.1 ? 'cc' : '44'}`,
                boxShadow: `0 0 ${40 * glow}px ${s.color}88`,
                opacity: e,
                transform: `translateX(${(1 - e) * 120}px) scale(${0.9 + e * 0.1})`,
              }}
            >
              <div style={{display: 'flex', alignItems: 'center', gap: 14, marginBottom: 10}}>
                <span style={{fontFamily: mono, fontSize: 18, color: C.dim}}>s{i + 1}</span>
                <Verb verb={s.verb} color={s.color} />
              </div>
              <div style={{fontFamily: mono, fontSize: 21, color: C.text}}>{s.body}</div>
            </div>
          );
        })}
      </div>
    </AbsoluteFill>
  );
};
