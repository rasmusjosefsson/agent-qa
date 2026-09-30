import React from 'react';
import {AbsoluteFill, Easing} from 'remotion';
import {TransitionSeries, linearTiming} from '@remotion/transitions';
import {fade} from '@remotion/transitions/fade';
import {slide} from '@remotion/transitions/slide';
import {wipe} from '@remotion/transitions/wipe';
import {flip} from '@remotion/transitions/flip';
import {Background} from './components/Background';
import {Hud} from './components/Hud';
import {Hook} from './scenes/Hook';
import {Problem} from './scenes/Problem';
import {Reveal} from './scenes/Reveal';
import {Record} from './scenes/Record';
import {Replay} from './scenes/Replay';
import {Heal} from './scenes/Heal';
import {Compare} from './scenes/Compare';
import {Outro} from './scenes/Outro';
import {sans} from './theme';

const T = 12;
const timing = linearTiming({durationInFrames: T, easing: Easing.inOut(Easing.cubic)});

export const SCENES = [90, 120, 110, 165, 130, 135, 115, 119];
export const TOTAL = SCENES.reduce((a, b) => a + b, 0) - T * (SCENES.length - 1);

export const Showreel: React.FC = () => (
  <AbsoluteFill style={{fontFamily: sans}}>
    <Background />
    <TransitionSeries>
      <TransitionSeries.Sequence durationInFrames={SCENES[0]}><Hook /></TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={fade()} timing={timing} />
      <TransitionSeries.Sequence durationInFrames={SCENES[1]}><Problem /></TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={wipe({direction: 'from-left'})} timing={timing} />
      <TransitionSeries.Sequence durationInFrames={SCENES[2]}><Reveal /></TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={slide({direction: 'from-bottom'})} timing={timing} />
      <TransitionSeries.Sequence durationInFrames={SCENES[3]}><Record /></TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={slide({direction: 'from-right'})} timing={timing} />
      <TransitionSeries.Sequence durationInFrames={SCENES[4]}><Replay /></TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={flip({direction: 'from-right'})} timing={timing} />
      <TransitionSeries.Sequence durationInFrames={SCENES[5]}><Heal /></TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={wipe({direction: 'from-top-right'})} timing={timing} />
      <TransitionSeries.Sequence durationInFrames={SCENES[6]}><Compare /></TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={fade()} timing={timing} />
      <TransitionSeries.Sequence durationInFrames={SCENES[7]}><Outro /></TransitionSeries.Sequence>
    </TransitionSeries>
    <Hud />
  </AbsoluteFill>
);
