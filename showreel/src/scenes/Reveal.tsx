import React from 'react';
import {AbsoluteFill, Easing, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {C, clamp, mono, sans} from '../theme';
import {LogoMark} from '../components/LogoMark';

const WORDS: [string, string][] = [
  ['Record.', C.red],
  ['Replay.', C.green],
  ['Compare.', C.cyan],
];

export const Reveal: React.FC = () => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  const draw = interpolate(f, [0, 26], [0, 1], {...clamp, easing: Easing.out(Easing.cubic)});
  const morph = spring({frame: f - 30, fps, config: {damping: 10}});
  const logoIn = spring({frame: f, fps, config: {damping: 11}});
  const slide = interpolate(f, [34, 54], [0, 1], {...clamp, easing: Easing.inOut(Easing.cubic)});
  const name = 'agent-qa';
  return (
    <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center'}}>
      {[0, 7, 14].map((d) => {
        const t = interpolate(f, [d, d + 40], [0, 1], clamp);
        return (
          <div
            key={d}
            style={{
              position: 'absolute',
              width: 300,
              height: 300,
              borderRadius: '50%',
              border: `2px solid ${C.cyan}`,
              transform: `scale(${0.5 + t * 5})`,
              opacity: (1 - t) * 0.6,
            }}
          />
        );
      })}
      <div style={{display: 'flex', alignItems: 'center', gap: 40, transform: `translateY(-70px)`}}>
        <div style={{transform: `scale(${logoIn}) translateX(${(1 - slide) * 290}px)`}}>
          <LogoMark size={220} draw={draw} morph={morph} id="reveal" />
        </div>
        <div style={{display: 'flex', fontFamily: sans, fontWeight: 900, fontSize: 200, letterSpacing: -8, color: C.text, overflow: 'hidden', paddingBottom: 20}}>
          {name.split('').map((ch, i) => {
            const s = spring({frame: f - 40 - i * 2, fps, config: {damping: 12, mass: 0.6}});
            return (
              <span
                key={i}
                style={{
                  display: 'inline-block',
                  transform: `translateY(${(1 - s) * 200}px) rotate(${(1 - s) * 20}deg)`,
                  opacity: s,
                  color: i >= 6 ? C.cyan : C.text,
                }}
              >
                {ch}
              </span>
            );
          })}
        </div>
      </div>
      <div style={{position: 'absolute', top: 700, display: 'flex', gap: 44, fontFamily: sans, fontWeight: 800, fontSize: 62}}>
        {WORDS.map(([w, col], i) => {
          const s = spring({frame: f - 62 - i * 8, fps, config: {damping: 10}});
          return (
            <span key={w} style={{color: col, opacity: s, transform: `translateY(${(1 - s) * 40}px) scale(${0.7 + s * 0.3})`, textShadow: `0 0 40px ${col}66`}}>
              {w}
            </span>
          );
        })}
      </div>
      <div style={{position: 'absolute', top: 810, fontFamily: mono, fontSize: 28, color: C.dim, letterSpacing: 2, opacity: interpolate(f, [88, 100], [0, 1], clamp)}}>
        end-to-end QA in a real browser — built for AI agents
      </div>
    </AbsoluteFill>
  );
};
