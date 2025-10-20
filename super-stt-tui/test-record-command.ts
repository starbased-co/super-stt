#!/usr/bin/env tsx

import { createRequire } from 'node:module';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));
const nativeModulePath = join(__dirname, 'super-stt-tui-native.linux-x64-gnu.node');
const { UdpClient } = require(nativeModulePath);

async function testRecordCommand() {
  console.log('Testing record command with native UDP client...\n');

  const client = new UdpClient();

  try {
    // Step 1: Connect to UDP server
    console.log('Step 1: Connecting to UDP server...');
    const response = await client.connect('tui');
    console.log(`✓ ${response}\n`);

    // Step 2: Send record command
    console.log('Step 2: Sending record command...');
    const recordResponse = await client.sendRecordCommand(undefined, true);
    console.log(`✓ ${recordResponse}\n`);

    console.log('Step 3: Listening for UDP packets for 15 seconds...');
    console.log('(Daemon will broadcast recording state, audio levels, and transcription)\n');

    // Step 3: Receive UDP packets in a loop
    let packetsReceived = 0;
    const startTime = Date.now();
    const duration = 15000; // 15 seconds

    while (Date.now() - startTime < duration) {
      try {
        const buffer = await client.receivePacket();
        packetsReceived++;

        const packetType = buffer[0];

        switch (packetType) {
          case 5: // Recording state
            const isRecording = buffer[11] === 1;
            console.log(`[${packetsReceived}] Recording state: ${isRecording ? 'ACTIVE 🔴' : 'IDLE ⚪'}`);
            break;

          case 6: // Frequency bands
            const totalEnergy = buffer.readFloatLE(15);
            const bar = '█'.repeat(Math.floor(totalEnergy * 50));
            console.log(`[${packetsReceived}] Audio level: ${bar} (${totalEnergy.toFixed(3)})`);
            break;

          case 3: // Final STT
            const confidence = buffer.readFloatLE(11);
            const text = buffer.slice(15).toString('utf-8');
            console.log(`[${packetsReceived}] Transcription: "${text}" (confidence: ${confidence.toFixed(2)})`);
            break;

          default:
            console.log(`[${packetsReceived}] Unknown packet type: ${packetType}`);
        }
      } catch (err: any) {
        if (Date.now() - startTime >= duration) break;
        console.error('Error receiving packet:', err.message);
        break;
      }
    }

    client.disconnect();
    console.log(`\n✓ Test complete! Received ${packetsReceived} packets`);
    process.exit(0);
  } catch (error: any) {
    console.error('Test failed:', error.message || error);
    process.exit(1);
  }
}

testRecordCommand();
