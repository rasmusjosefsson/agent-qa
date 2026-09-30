import {loadFont as loadInter} from '@remotion/google-fonts/Inter';
import {loadFont as loadMono} from '@remotion/google-fonts/JetBrainsMono';

export const {fontFamily: sans} = loadInter('normal', {
  weights: ['400', '600', '800', '900'],
  subsets: ['latin'],
});
export const {fontFamily: mono} = loadMono('normal', {
  weights: ['400', '700'],
  subsets: ['latin'],
});

export const C = {
  bg: '#06070c',
  panel: '#0e1120',
  panelHi: '#151a2e',
  line: 'rgba(255,255,255,0.08)',
  text: '#f5f7ff',
  dim: '#8a93b2',
  red: '#ff4d6d',
  green: '#2ef59a',
  cyan: '#38d6ff',
  violet: '#8b5cf6',
  amber: '#ffc53d',
};

export const clamp = {
  extrapolateLeft: 'clamp',
  extrapolateRight: 'clamp',
} as const;

export const FPS = 30;
