import React from 'react';
import {Composition} from 'remotion';
import {Showreel, TOTAL} from './Showreel';
import {FPS} from './theme';

export const RemotionRoot: React.FC = () => (
  <Composition id="Showreel" component={Showreel} durationInFrames={TOTAL} fps={FPS} width={1920} height={1080} />
);
