#!/usr/bin/env tsx

import { createNativeUdpClient } from './src/protocol/native-client.js';

async function test() {
  console.log('Testing TUI connection flow...\n');

  try {
    console.log('1. Creating native UDP client...');
    const client = await createNativeUdpClient('tui-test');
    console.log('✅ Client created and connected');

    client.on('connected', () => {
      console.log('✅ Event: connected');
    });

    client.on('registered', (clientId) => {
      console.log('✅ Event: registered -', clientId);
    });

    client.on('error', (error) => {
      console.error('❌ Event: error -', error.message);
    });

    // Wait a bit for events
    await new Promise(resolve => setTimeout(resolve, 2000));

    console.log('\n2. Disconnecting...');
    client.disconnect();
    console.log('✅ Disconnected cleanly');

    console.log('\n✅✅✅ TUI CONNECTION TEST PASSED ✅✅✅');

  } catch (error) {
    console.error('\n❌ Test failed:', (error as Error).message);
    console.error((error as Error).stack);
    process.exit(1);
  }
}

test();
