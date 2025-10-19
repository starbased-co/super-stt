#!/usr/bin/env tsx
/**
 * Real-time Audio Capture and Transcription Test
 *
 * Tests the complete flow:
 * 1. UDP client connection
 * 2. Real-time transcription session start
 * 3. Audio capture from microphone
 * 4. Audio streaming to daemon
 * 5. UDP packet reception (visualization + transcription)
 */

import { createRequire } from 'node:module';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));
const nativeModulePath = join(__dirname, 'super-stt-tui-native.linux-x64-gnu.node');
const { UdpClient, AudioCapture } = require(nativeModulePath);

console.log('🎤 Super STT Real-time Audio Test\n');

// Test configuration
const TEST_DURATION_MS = 10000; // 10 seconds
const SOCKET_PATH = `/run/user/${process.getuid()}/stt/super-stt.sock`;

interface TestStats {
  udpPacketsReceived: number;
  recordingStatePackets: number;
  frequencyBandPackets: number;
  audioSamplePackets: number;
  partialTranscriptions: number;
  finalTranscriptions: number;
  errors: string[];
}

const stats: TestStats = {
  udpPacketsReceived: 0,
  recordingStatePackets: 0,
  frequencyBandPackets: 0,
  audioSamplePackets: 0,
  partialTranscriptions: 0,
  finalTranscriptions: 0,
  errors: [],
};

async function parsePacket(buffer: Buffer) {
  if (buffer.length < 5) {
    return null;
  }

  const packetType = buffer.readUInt8(4);
  stats.udpPacketsReceived++;

  switch (packetType) {
    case 2: // Partial STT
      stats.partialTranscriptions++;
      const partialLen = buffer.readUInt32LE(5);
      const partialText = buffer.subarray(13, 13 + partialLen).toString('utf-8');
      console.log(`📝 Partial: "${partialText}"`);
      return { type: 'partial_stt', text: partialText };

    case 3: // Final STT
      stats.finalTranscriptions++;
      const finalLen = buffer.readUInt32LE(5);
      const finalText = buffer.subarray(13, 13 + finalLen).toString('utf-8');
      console.log(`✅ Final: "${finalText}"`);
      return { type: 'final_stt', text: finalText };

    case 4: // Audio Samples
      stats.audioSamplePackets++;
      return { type: 'audio_samples' };

    case 5: // Recording State
      stats.recordingStatePackets++;
      const isRecording = buffer.readUInt8(5) === 1;
      console.log(`🔴 Recording: ${isRecording ? 'ACTIVE' : 'IDLE'}`);
      return { type: 'recording_state', isRecording };

    case 6: // Frequency Bands
      stats.frequencyBandPackets++;
      const bandCount = buffer.readUInt32LE(9);
      if (stats.frequencyBandPackets % 10 === 0) {
        console.log(`📊 Frequency bands: ${bandCount} bands`);
      }
      return { type: 'frequency_bands', bandCount };

    default:
      return null;
  }
}

async function runTest() {
  console.log('Step 1: Initializing UDP client...');
  const udpClient = new UdpClient();

  try {
    const response = await udpClient.connect('tui-test');
    console.log(`✅ Connected: ${response}\n`);
  } catch (error) {
    console.error('❌ Failed to connect UDP client:', error);
    stats.errors.push(`UDP connection failed: ${error}`);
    return;
  }

  console.log('Step 2: Starting real-time transcription session...');
  try {
    await udpClient.startRealtimeTranscription(SOCKET_PATH);
    console.log(`✅ Real-time session started\n`);
  } catch (error) {
    console.error('❌ Failed to start real-time session:', error);
    stats.errors.push(`Real-time session failed: ${error}`);
    udpClient.disconnect();
    return;
  }

  console.log('Step 3: Starting audio capture...');
  const audioCapture = new AudioCapture();

  try {
    audioCapture.startCapture(udpClient, SOCKET_PATH);
    console.log(`✅ Audio capture started\n`);
    console.log('🎙️  Speak into your microphone now!\n');
  } catch (error) {
    console.error('❌ Failed to start audio capture:', error);
    stats.errors.push(`Audio capture failed: ${error}`);
    udpClient.disconnect();
    return;
  }

  console.log('Step 4: Monitoring UDP packets for visualization data...\n');

  // Start packet receiver
  const packetReceiver = (async () => {
    while (udpClient.isConnected()) {
      try {
        const packet = await udpClient.receivePacket();
        await parsePacket(packet);
      } catch (error) {
        if (udpClient.isConnected()) {
          console.error('Error receiving packet:', error);
          stats.errors.push(`Packet receive error: ${error}`);
        }
        break;
      }
    }
  })();

  // Wait for test duration
  await new Promise(resolve => setTimeout(resolve, TEST_DURATION_MS));

  console.log('\n\nStep 5: Stopping capture and cleaning up...');
  audioCapture.stopCapture();
  udpClient.disconnect();

  // Wait for receiver to finish
  await Promise.race([
    packetReceiver,
    new Promise(resolve => setTimeout(resolve, 1000))
  ]);

  console.log('✅ Test completed\n');
}

async function printResults() {
  console.log('═══════════════════════════════════════');
  console.log('           TEST RESULTS');
  console.log('═══════════════════════════════════════\n');

  console.log('📊 Statistics:');
  console.log(`  Total UDP packets received: ${stats.udpPacketsReceived}`);
  console.log(`  Recording state updates:    ${stats.recordingStatePackets}`);
  console.log(`  Frequency band packets:     ${stats.frequencyBandPackets}`);
  console.log(`  Audio sample packets:       ${stats.audioSamplePackets}`);
  console.log(`  Partial transcriptions:     ${stats.partialTranscriptions}`);
  console.log(`  Final transcriptions:       ${stats.finalTranscriptions}\n`);

  if (stats.errors.length > 0) {
    console.log('❌ Errors encountered:');
    stats.errors.forEach((error, i) => {
      console.log(`  ${i + 1}. ${error}`);
    });
    console.log();
  }

  console.log('✅ Verification Results:\n');

  const checks = [
    {
      name: 'UDP connection established',
      pass: stats.udpPacketsReceived > 0,
      expected: '> 0 packets',
      actual: `${stats.udpPacketsReceived} packets`,
    },
    {
      name: 'Recording state broadcast',
      pass: stats.recordingStatePackets > 0,
      expected: '> 0 updates',
      actual: `${stats.recordingStatePackets} updates`,
    },
    {
      name: 'Frequency visualization data',
      pass: stats.frequencyBandPackets > 0,
      expected: '> 0 packets',
      actual: `${stats.frequencyBandPackets} packets`,
    },
    {
      name: 'Audio capture working',
      pass: stats.frequencyBandPackets >= 50,
      expected: '≥ 50 packets (5 sec @ 10Hz)',
      actual: `${stats.frequencyBandPackets} packets`,
    },
    {
      name: 'Transcription attempted',
      pass: stats.partialTranscriptions > 0 || stats.finalTranscriptions > 0,
      expected: '> 0 transcriptions',
      actual: `${stats.partialTranscriptions + stats.finalTranscriptions} total`,
    },
  ];

  checks.forEach(check => {
    const icon = check.pass ? '✅' : '❌';
    const status = check.pass ? 'PASS' : 'FAIL';
    console.log(`  ${icon} ${check.name}: ${status}`);
    console.log(`     Expected: ${check.expected}`);
    console.log(`     Actual:   ${check.actual}\n`);
  });

  const allPassed = checks.every(c => c.pass);

  console.log('═══════════════════════════════════════');
  if (allPassed) {
    console.log('🎉 ALL CHECKS PASSED - Audio streaming is working!');
  } else {
    console.log('⚠️  SOME CHECKS FAILED - Review results above');
  }
  console.log('═══════════════════════════════════════\n');

  process.exit(allPassed ? 0 : 1);
}

// Run test
console.log('Starting test in 2 seconds...\n');
setTimeout(async () => {
  try {
    await runTest();
  } catch (error) {
    console.error('\n❌ Test failed with error:', error);
    stats.errors.push(`Test failure: ${error}`);
  } finally {
    await printResults();
  }
}, 2000);
