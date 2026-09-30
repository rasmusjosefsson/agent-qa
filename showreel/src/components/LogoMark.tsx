import React from 'react';
import {C} from '../theme';

export const LogoMark: React.FC<{size: number; draw: number; morph: number; id: string}> = ({
  size,
  draw,
  morph,
  id,
}) => {
  const c = size / 2;
  const r = c - 8;
  const circ = 2 * Math.PI * r;
  const s = size;
  const tri = `M ${-0.14 * s} ${-0.19 * s} L ${0.22 * s} 0 L ${-0.14 * s} ${0.19 * s} Z`;
  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} style={{overflow: 'visible'}}>
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor={C.cyan} />
          <stop offset="1" stopColor={C.violet} />
        </linearGradient>
      </defs>
      <circle
        cx={c}
        cy={c}
        r={r}
        fill="none"
        stroke={`url(#${id})`}
        strokeWidth={10}
        strokeDasharray={circ}
        strokeDashoffset={circ * (1 - draw)}
        strokeLinecap="round"
        transform={`rotate(-90 ${c} ${c})`}
        style={{filter: `drop-shadow(0 0 14px ${C.cyan})`}}
      />
      <circle cx={c} cy={c} r={Math.max(0, s * 0.2 * (1 - morph))} fill={C.red} style={{filter: `drop-shadow(0 0 18px ${C.red})`}} />
      <path
        d={tri}
        fill={C.green}
        transform={`translate(${c + 0.02 * s} ${c}) rotate(${(1 - morph) * -120}) scale(${morph})`}
        style={{filter: `drop-shadow(0 0 18px ${C.green})`}}
      />
    </svg>
  );
};
