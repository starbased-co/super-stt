/**
 * Application header with branding
 */

import React from 'react';
import { Box, Text } from 'ink';

export const Header: React.FC = () => {
  return (
    <Box flexDirection="column">
      <Box>
        <Text bold color="cyan">
          ╔════════════════════════════════════════════════════════════╗
        </Text>
      </Box>
      <Box>
        <Text bold color="cyan">
          ║           Super STT - Terminal Interface                  ║
        </Text>
      </Box>
      <Box marginBottom={1}>
        <Text bold color="cyan">
          ╚════════════════════════════════════════════════════════════╝
        </Text>
      </Box>
    </Box>
  );
};
