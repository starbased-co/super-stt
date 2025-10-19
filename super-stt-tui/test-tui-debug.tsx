#!/usr/bin/env tsx

import React, { useState, useEffect } from 'react';
import { render, Text, Box } from 'ink';
import { NativeUdpClient } from './src/protocol/native-client.js';

function DebugApp() {
  const [status, setStatus] = useState('initializing...');
  const [clientId, setClientId] = useState<string | null>(null);

  useEffect(() => {
    let client: NativeUdpClient | null = null;
    let mounted = true;

    async function connect() {
      try {
        console.error('[DEBUG] Creating client...');
        client = new NativeUdpClient();

        console.error('[DEBUG] Attaching listeners...');
        client.on('connected', () => {
          console.error('[DEBUG] Event: connected');
          if (mounted) setStatus('connected');
        });

        client.on('registered', (id) => {
          console.error('[DEBUG] Event: registered -', id);
          if (mounted) setClientId(id);
        });

        client.on('error', (error) => {
          console.error('[DEBUG] Event: error -', error.message);
          if (mounted) setStatus(`error: ${error.message}`);
        });

        console.error('[DEBUG] Calling connect()...');
        await client.connect('tui-debug');
        console.error('[DEBUG] connect() returned successfully');

      } catch (error) {
        console.error('[DEBUG] Exception:', error);
        if (mounted) setStatus(`exception: ${(error as Error).message}`);
      }
    }

    connect();

    return () => {
      mounted = false;
      client?.disconnect();
    };
  }, []);

  return (
    <Box flexDirection="column" padding={1}>
      <Text bold>Debug TUI Test</Text>
      <Text>Status: {status}</Text>
      <Text>Client ID: {clientId || 'none'}</Text>
    </Box>
  );
}

const { unmount } = render(<DebugApp />);

setTimeout(() => {
  console.error('\n[DEBUG] Unmounting after 3 seconds...');
  unmount();
}, 3000);
