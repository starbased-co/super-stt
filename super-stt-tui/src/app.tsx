import React from 'react';
import { Box, Text } from 'ink';

interface AppProps {
  // Props can be added here later
}

/**
 * Main application component for Super STT TUI
 */
export const App: React.FC<AppProps> = () => {
  return (
    <Box flexDirection="column" paddingY={1}>
      {/* Header */}
      <Box marginBottom={1}>
        <Text bold color="cyan">
          ╔════════════════════════════════════════╗
        </Text>
      </Box>
      <Box>
        <Text bold color="cyan">
          ║     Super STT Terminal Interface      ║
        </Text>
      </Box>
      <Box marginBottom={1}>
        <Text bold color="cyan">
          ╚════════════════════════════════════════╝
        </Text>
      </Box>

      {/* Connection Status */}
      <Box marginY={1}>
        <Text>
          <Text color="yellow">●</Text> Connection Status:{' '}
          <Text dimColor>Not Connected</Text>
        </Text>
      </Box>

      {/* Placeholder for Audio Visualization */}
      <Box marginY={1} flexDirection="column">
        <Text bold>Audio Visualization</Text>
        <Text dimColor>
          [Audio meters will appear here when connected]
        </Text>
      </Box>

      {/* Placeholder for Transcription */}
      <Box marginY={1} flexDirection="column">
        <Text bold>Transcription</Text>
        <Text dimColor>
          [Real-time transcription will appear here]
        </Text>
      </Box>

      {/* Footer with instructions */}
      <Box marginTop={2}>
        <Text dimColor>
          Press <Text bold>q</Text> to quit • <Text bold>Space</Text> to start/stop recording
        </Text>
      </Box>
    </Box>
  );
};

export default App;