import React from 'react';
import { Box, Text, useInput, useApp } from 'ink';
import { useUdpClient } from './hooks/useUdpClient.js';
import { Header } from './components/Header.js';
import { ConnectionStatus } from './components/ConnectionStatus.js';
import { RecordingIndicator } from './components/RecordingIndicator.js';
import { AudioMeter } from './components/AudioMeter.js';
import { FrequencyVisualizer } from './components/FrequencyVisualizer.js';
import { TranscriptionDisplay } from './components/TranscriptionDisplay.js';

interface AppProps {
  // Props can be added here later
}

/**
 * Main application component for Super STT TUI
 */
export const App: React.FC<AppProps> = () => {
  const { exit } = useApp();

  const {
    isConnected,
    isRegistered,
    isRecording,
    audioLevel,
    freqBands,
    sampleRate,
    partialText,
    partialConfidence,
    finalText,
    finalConfidence,
    error,
    clientId,
    startRecording,
  } = useUdpClient();

  // Handle keyboard input
  useInput((input) => {
    if (input === 'q' || input === 'Q') {
      exit();
    } else if (input === 'r' || input === 'R') {
      if (isRegistered && !isRecording) {
        startRecording().catch((err) => {
          console.error('Failed to start recording:', err);
        });
      }
    }
  });

  return (
    <Box flexDirection="column" paddingY={1} paddingX={2}>
      <Header />

      {/* Connection Status and Recording Indicator */}
      <Box flexDirection="column" marginY={1}>
        <ConnectionStatus
          isConnected={isConnected}
          isRegistered={isRegistered}
          clientId={clientId}
          error={error}
        />
        <Box marginTop={1}>
          <RecordingIndicator isRecording={isRecording} />
        </Box>
      </Box>

      {/* Audio Visualization */}
      {isRegistered && (
        <Box flexDirection="column" marginY={1}>
          <AudioMeter level={audioLevel} isRecording={isRecording} />
        </Box>
      )}

      {/* Frequency Spectrum */}
      {isRegistered && freqBands.length > 0 && (
        <Box flexDirection="column" marginY={1}>
          <FrequencyVisualizer bands={freqBands} sampleRate={sampleRate} />
        </Box>
      )}

      {/* Transcription Display */}
      <Box flexDirection="column" marginY={1}>
        <TranscriptionDisplay
          partialText={partialText}
          partialConfidence={partialConfidence}
          finalText={finalText}
          finalConfidence={finalConfidence}
        />
      </Box>

      {/* Footer with instructions */}
      <Box marginTop={2} borderStyle="single" borderColor="gray" paddingX={1}>
        <Text dimColor>
          Press <Text bold>r</Text> to record | <Text bold>q</Text> to quit
        </Text>
      </Box>
    </Box>
  );
};

export default App;