/**
 * Recording state indicator
 *
 * Large visual indicator showing if system is recording
 */

import React from 'react';
import { Box, Text } from 'ink';

interface RecordingIndicatorProps {
  isRecording: boolean;
}

export const RecordingIndicator: React.FC<RecordingIndicatorProps> = ({
  isRecording,
}) => {
  if (!isRecording) {
    return (
      <Box>
        <Text dimColor>⚪ Idle</Text>
      </Box>
    );
  }

  return (
    <Box>
      <Text color="red" bold>
        🔴 RECORDING
      </Text>
    </Box>
  );
};
