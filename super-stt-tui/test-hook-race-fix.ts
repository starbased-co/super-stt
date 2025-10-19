#!/usr/bin/env tsx

import { NativeUdpClient } from './src/protocol/native-client.js';

async function test() {
  console.log('Testing race condition fix...\n');

  const client = new NativeUdpClient();
  let connectedFired = false;
  let registeredFired = false;

  // Attach listeners FIRST
  client.on('connected', () => {
    console.log('✅ Event: connected');
    connectedFired = true;
  });

  client.on('registered', (clientId) => {
    console.log('✅ Event: registered -', clientId);
    registeredFired = true;
  });

  client.on('error', (error) => {
    console.error('❌ Event: error -', error.message);
  });

  // THEN connect
  console.log('Connecting...');
  await client.connect('race-test');

  // Wait for async event emission
  await new Promise(resolve => setTimeout(resolve, 100));

  console.log('\nResults:');
  console.log('  Connected event fired:', connectedFired);
  console.log('  Registered event fired:', registeredFired);

  client.disconnect();

  if (connectedFired && registeredFired) {
    console.log('\n✅✅✅ RACE CONDITION FIXED! ✅✅✅');
  } else {
    console.log('\n❌ Race condition still exists');
    process.exit(1);
  }
}

test().catch(err => {
  console.error('Test failed:', err);
  process.exit(1);
});
