/**
 * Transcription display component
 *
 * Shows:
 * - Partial (in-progress) transcription in dim text
 * - Final transcription in bold green text
 */

import React from 'react';
import { Box, Text } from 'ink';

interface TranscriptionDisplayProps {
  partialText: string;
  partialConfidence: number;
  finalText: string;
  finalConfidence: number;
}

export const TranscriptionDisplay: React.FC<TranscriptionDisplayProps> = ({
  partialText,
  partialConfidence,
  finalText,
  finalConfidence,
}) => {
  const hasPartial = partialText.length > 0;
  const hasFinal = finalText.length > 0;

  console.log('[TranscriptionDisplay] Render:', {
    partialText,
    partialTextLength: partialText.length,
    hasPartial,
    finalText,
    finalTextLength: finalText.length,
    hasFinal,
    partialConfidence,
    finalConfidence,
  });

  return (
    <Box flexDirection="column">
      <Text bold>Transcription</Text>

      {!hasPartial && !hasFinal && (
        <Box marginTop={1}>
          <Text dimColor>Start speaking to see transcription...</Text>
        </Box>
      )}

      {hasPartial && (
        <Box marginTop={1} flexDirection="column">
          <Box>
            <Text dimColor>Transcribing... </Text>
            <Text dimColor>({(partialConfidence * 100).toFixed(0)}%)</Text>
          </Box>
          <Box>
            <Text italic dimColor>
              {partialText}
            </Text>
          </Box>
        </Box>
      )}

      {hasFinal && (
        <Box marginTop={1} flexDirection="column">
          <Box>
            <Text color="green">Result: </Text>
            <Text dimColor>({(finalConfidence * 100).toFixed(0)}%)</Text>
          </Box>
          <Box>
            <Text color="green" bold>
              {finalText}
            </Text>
          </Box>
        </Box>
      )}
    </Box>
  );
};
