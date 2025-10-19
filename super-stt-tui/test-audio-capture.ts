#!/usr/bin/env tsx

/**
 * Test script for audio capture and streaming
 *
 * This verifies:
 * 1. AudioCapture can be instantiated
 * 2. UdpClient connects successfully
 * 3. Audio streaming starts without errors
 * 4. Graceful shutdown works
 */

import { AudioCapture, UdpClient } from './super-stt-tui-native.linux-x64-gnu.node';

async function main() {
  console.log('🎨microphone Starting audio capture test...\n');

  const udpClient = new UdpClient();
  console.log('  Created UDP client');

  try {
    const clientId = await udpClient.connect('tui-test');
    console.log(`  Connected to daemon: ${clientId}`);
  } catch (err) {
    console.error('  Failed to connect to daemon:', err);
    console.log('\n  Make sure super-stt daemon is running:');
    console.log('  systemctl --user start super-stt');
    process.exit(1);
  }

  try {
    await udpClient.startRealtimeTranscription();
    console.log('  Started realtime transcription session');
  } catch (err) {
    console.error('  Failed to start transcription:', err);
    process.exit(1);
  }

  const audioCapture = new AudioCapture();
  console.log('  Created audio capture instance');

  try {
    audioCapture.startCapture(udpClient);
    console.log('  Audio capture started');
    console.log(`  Recording: ${audioCapture.isRecording()}`);
  } catch (err) {
    console.error('  Failed to start audio capture:', err);
    console.log('\n  Possible issues:');
    console.log('  - No microphone available');
    console.log('  - PipeWire/PulseAudio permissions');
    console.log('  - Audio device in use');
    process.exit(1);
  }

  console.log('\n󰓃  Recording for 5 seconds...\n');
  console.log('  Speak into your microphone. You should see debug logs if RUST_LOG=debug is set.');

  let packetCount = 0;
  const receiveLoop = async () => {
    try {
      while (audioCapture.isRecording()) {
        const packet = await udpClient.receivePacket();
        packetCount++;

        const text = Buffer.from(packet).toString('utf-8');

        if (text.startsWith('TRANSCRIPTION:')) {
          console.log(`🎨speech-bubble ${text}`);
        } else if (text.startsWith('PARTIAL:')) {
          process.stdout.write(`\r🎨clock ${text.replace('PARTIAL:', '').trim()}    `);
        } else if (text.startsWith('VIZ:')) {
          if (packetCount % 10 === 0) {
            process.stdout.write('.');
          }
        }
      }
    } catch (err) {
      if (audioCapture.isRecording()) {
        console.error('\n  Error receiving packet:', err);
      }
    }
  };

  receiveLoop();

  await new Promise(resolve => setTimeout(resolve, 5000));

  console.log('\n\n🎨stop-circle Stopping audio capture...');
  audioCapture.stopCapture();
  console.log(`  Recording stopped: ${!audioCapture.isRecording()}`);

  udpClient.disconnect();
  console.log('  Disconnected from daemon');

  console.log(`\n🎨checkmark Test completed! Received ${packetCount} packets`);
}

main().catch(err => {
  console.error('  Fatal error:', err);
  process.exit(1);
});
