/**
 * Connection status indicator
 *
 * Shows:
 * - Red: Disconnected/Error
 * - Yellow: Connected but not registered
 * - Green: Connected and registered
 */

import React from 'react';
import { Box, Text } from 'ink';

interface ConnectionStatusProps {
  isConnected: boolean;
  isRegistered: boolean;
  clientId: string | null;
  error: Error | null;
}

export const ConnectionStatus: React.FC<ConnectionStatusProps> = ({
  isConnected,
  isRegistered,
  clientId,
  error,
}) => {
  let statusColor: 'red' | 'yellow' | 'green';
  let statusText: string;
  let indicator: string;

  if (error) {
    statusColor = 'red';
    statusText = `Error: ${error.message}`;
    indicator = '●';
  } else if (!isConnected) {
    statusColor = 'red';
    statusText = 'Disconnected';
    indicator = '●';
  } else if (!isRegistered) {
    statusColor = 'yellow';
    statusText = 'Connecting...';
    indicator = '●';
  } else {
    statusColor = 'green';
    statusText = `Connected: ${clientId || 'unknown'}`;
    indicator = '●';
  }

  return (
    <Box>
      <Text color={statusColor}>{indicator}</Text>
      <Text> Status: </Text>
      <Text color={statusColor}>{statusText}</Text>
    </Box>
  );
};
