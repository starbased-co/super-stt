#!/usr/bin/env tsx

import React from 'react';
import { render, Box, Text } from 'ink';
import { useUdpClient } from './src/hooks/useUdpClient.js';

function MinimalApp() {
  const state = useUdpClient();

  return (
    <Box flexDirection="column" padding={1}>
      <Text bold color="cyan">Minimal TUI Test</Text>
      <Text>isConnected: {String(state.isConnected)}</Text>
      <Text>isRegistered: {String(state.isRegistered)}</Text>
      <Text>clientId: {state.clientId || 'null'}</Text>
      <Text>error: {state.error?.message || 'null'}</Text>
    </Box>
  );
}

const { unmount } = render(<MinimalApp />);

// Run for 5 seconds
setTimeout(() => {
  console.error('\n[INFO] Unmounting...');
  unmount();
}, 5000);
