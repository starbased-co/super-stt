/**
 * VU meter for overall audio level
 *
 * Displays horizontal bar showing totalEnergy from frequency bands
 */

import React from 'react';
import { Box, Text } from 'ink';

interface AudioMeterProps {
  level: number; // 0.0 - 1.0
  isRecording: boolean;
}

const METER_WIDTH = 50;

export const AudioMeter: React.FC<AudioMeterProps> = ({ level, isRecording }) => {
  // Clamp level to 0-1 range
  const clampedLevel = Math.max(0, Math.min(1, level));

  // Calculate bar length
  const barLength = Math.floor(clampedLevel * METER_WIDTH);

  // Create bar with color gradient
  const bar = '█'.repeat(barLength);
  const empty = '░'.repeat(METER_WIDTH - barLength);

  // Color based on level
  let barColor: 'green' | 'yellow' | 'red' = 'green';
  if (clampedLevel > 0.7) {
    barColor = 'red';
  } else if (clampedLevel > 0.4) {
    barColor = 'yellow';
  }

  // Format percentage
  const percentage = (clampedLevel * 100).toFixed(1);

  return (
    <Box flexDirection="column">
      <Box>
        <Text bold>Audio Level</Text>
        {isRecording && (
          <Text color="red" bold>
            {' '}
            [RECORDING]
          </Text>
        )}
      </Box>
      <Box>
        <Text color={barColor}>{bar}</Text>
        <Text dimColor>{empty}</Text>
        <Text> </Text>
        <Text>{percentage}%</Text>
      </Box>
    </Box>
  );
};
