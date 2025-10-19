#!/usr/bin/env tsx
/**
 * Test UDP authentication directly
 */

import { createRequire } from 'node:module';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { readFileSync } from 'node:fs';

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));
const nativeModulePath = join(__dirname, 'super-stt-tui-native.linux-x64-gnu.node');
const { UdpClient } = require(nativeModulePath);

console.log('🔐 Testing UDP Authentication\n');

// Read the secret file directly
const secretPath = `/run/user/${process.getuid()}/stt/udp_secret`;
console.log(`Secret file: ${secretPath}`);

try {
  const secret = readFileSync(secretPath, 'utf-8').trim();
  console.log(`Secret content: ${secret}\n`);
} catch (error) {
  console.error('❌ Failed to read secret:', error);
  process.exit(1);
}

async function testConnection() {
  const client = new UdpClient();

  console.log('Creating UDP client and attempting connection...\n');

  try {
    const response = await client.connect('auth-test');
    console.log(`✅ Connection successful!`);
    console.log(`Response: ${response}\n`);

    // Test if we can receive packets
    console.log('Testing packet reception (5 second timeout)...');
    const timeout = setTimeout(() => {
      console.log('⏱️  Timeout - no packets received');
      client.disconnect();
      process.exit(0);
    }, 5000);

    try {
      const packet = await client.receivePacket();
      clearTimeout(timeout);
      console.log(`✅ Received packet: ${packet.length} bytes`);
      console.log(`Packet type: ${packet.readUInt8(4)}`);
      client.disconnect();
    } catch (error) {
      clearTimeout(timeout);
      console.error('❌ Error receiving packet:', error);
      client.disconnect();
    }
  } catch (error) {
    console.error('\n❌ Connection failed:');
    console.error(error);
    console.error('\nThis could mean:');
    console.error('1. Daemon is not running');
    console.error('2. UDP port 8765 is not accessible');
    console.error('3. Authentication secret mismatch');
    console.error('4. Daemon rejected the connection\n');
    process.exit(1);
  }
}

testConnection();
