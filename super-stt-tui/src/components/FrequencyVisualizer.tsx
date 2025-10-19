/**
 * Frequency spectrum visualizer
 *
 * Displays vertical bars for each frequency band
 */

import React from 'react';
import { Box, Text } from 'ink';

interface FrequencyVisualizerProps {
  bands: number[];
  sampleRate: number;
}

const MAX_BAR_HEIGHT = 12;

type BarColor = 'red' | 'yellow' | 'green' | 'dim';

export const FrequencyVisualizer: React.FC<FrequencyVisualizerProps> = ({
  bands,
  sampleRate,
}) => {
  if (bands.length === 0) {
    return (
      <Box flexDirection="column">
        <Text bold>Frequency Spectrum</Text>
        <Text dimColor>Waiting for audio data...</Text>
      </Box>
    );
  }

  // Create vertical bars for each band
  const bars: BarColor[][] = [];

  // Initialize bar columns
  for (let i = 0; i < bands.length; i++) {
    bars[i] = [];
  }

  // Fill bars from bottom to top
  for (let row = MAX_BAR_HEIGHT - 1; row >= 0; row--) {
    for (let col = 0; col < bands.length; col++) {
      const level = bands[col];
      const height = Math.floor(level * MAX_BAR_HEIGHT);

      if (row >= MAX_BAR_HEIGHT - height) {
        // Determine color based on height (inverted since we draw top-down)
        if (row < 4) {
          bars[col][row] = 'red'; // Top rows are red (loud)
        } else if (row < 8) {
          bars[col][row] = 'yellow'; // Middle rows are yellow
        } else {
          bars[col][row] = 'green'; // Bottom rows are green (quiet)
        }
      } else {
        bars[col][row] = 'dim'; // Empty space
      }
    }
  }

  return (
    <Box flexDirection="column">
      <Box>
        <Text bold>Frequency Spectrum</Text>
        <Text dimColor> ({sampleRate.toFixed(0)} Hz)</Text>
      </Box>
      <Box flexDirection="column">
        {bars[0]?.map((_, rowIndex) => (
          <Box key={rowIndex}>
            {bars.map((bar, colIndex) => {
              const barColor = bar[rowIndex];
              const isDim = barColor === 'dim';
              const color = isDim ? undefined : barColor;

              return (
                <Text key={colIndex} color={color} dimColor={isDim}>
                  {'█'}
                </Text>
              );
            })}
          </Box>
        ))}
      </Box>
      <Box>
        <Text dimColor>{'─'.repeat(bands.length)}</Text>
      </Box>
      <Box justifyContent="space-between">
        <Text dimColor>Low</Text>
        <Text dimColor>High</Text>
      </Box>
    </Box>
  );
};
