import React from 'react';
import {AbsoluteFill, Easing, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {C, clamp, mono, sans} from '../theme';
import {LogoMark} from '../components/LogoMark';
import {typed} from '../components/ui';

const AGENTS = ['Claude Code', 'Codex', 'Cursor', 'Gemini CLI', 'GitHub Copilot', 'Goose', 'OpenCode', 'Windsurf', 'Pi'];

export const Outro: React.FC = () => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  const cmd = typed('npm i -g @rasmusjosefsson/agent-qa', f, 4, 1.4);
  const term = spring({frame: f, fps, config: {damping: 14}});
  const up = interpolate(f, [48, 64], [0, 1], {...clamp, easing: Easing.inOut(Easing.cubic)});
  const logo = spring({frame: f - 50, fps, config: {damping: 11}});
  const tag = spring({frame: f - 66, fps, config: {damping: 12}});
  const fadeOut = interpolate(f, [108, 119], [0, 1], clamp);
  const marquee = AGENTS.concat(AGENTS).join('   ·   ');
  return (
    <AbsoluteFill style={{alignItems: 'center'}}>
      <div
        style={{
          position: 'absolute',
          top: 440 - up * 50,
          width: 1100,
          padding: '34px 40px',
          borderRadius: 20,
          background: '#070913ee',
          border: `1px solid ${C.line}`,
          fontFamily: mono,
          fontSize: 36,
          color: C.text,
          opacity: term * (1 - up * 0.0) ,
          transform: `translateY(${(1 - term) * 60 + up * 330}px) scale(${1 - up * 0.3})`,
          boxShadow: `0 30px 90px rgba(0,0,0,0.6)`,
        }}
      >
        <span style={{color: C.green}}>$ </span>
        {cmd}
        {f > 40 && <span style={{color: C.green, marginLeft: 24}}>✓</span>}
      </div>
      <div style={{position: 'absolute', top: 200, display: 'flex', alignItems: 'center', gap: 36, opacity: logo, transform: `scale(${0.6 + logo * 0.4})`}}>
        <LogoMark size={170} draw={1} morph={1} id="outro" />
        <div style={{fontFamily: sans, fontWeight: 900, fontSize: 170, letterSpacing: -7, color: C.text}}>
          agent<span style={{color: C.cyan}}>-qa</span>
        </div>
      </div>
      <div style={{position: 'absolute', top: 450, fontFamily: sans, fontWeight: 800, fontSize: 64, color: C.text, opacity: tag, transform: `translateY(${(1 - tag) * 30}px)`}}>
        Record once. <span style={{color: C.green}}>Trust every release.</span>
      </div>
      <div style={{position: 'absolute', top: 560, fontFamily: mono, fontSize: 28, color: C.dim, opacity: tag}}>github.com/rasmusjosefsson/agent-qa</div>
      <div style={{position: 'absolute', bottom: 110, left: 0, right: 0, overflow: 'hidden', opacity: tag * 0.8}}>
        <div style={{whiteSpace: 'nowrap', fontFamily: mono, fontSize: 26, color: C.violet, letterSpacing: 2, transform: `translateX(${-f * 6}px)`}}>
          WORKS WITH&nbsp;&nbsp; {marquee}
        </div>
      </div>
      <AbsoluteFill style={{background: '#000', opacity: fadeOut}} />
    </AbsoluteFill>
  );
};
