import dgram from 'node:dgram';
import { readFileSync } from 'node:fs';

const socket = dgram.createSocket('udp4');

// Read secret
const secret = readFileSync('/run/user/1000/stt/udp_secret', 'utf-8').trim();
console.log('Secret:', secret);

// Register handler
socket.on('message', (msg, rinfo) => {
  console.log(`✅ Received from ${rinfo.address}:${rinfo.port}: ${msg.toString()}`);
  process.exit(0);
});

socket.on('error', (err) => {
  console.error('Socket error:', err);
  process.exit(1);
});

// Send registration
const regMsg = `REGISTER:tsx_test:${secret}`;
console.log('Sending:', regMsg);

const buffer = Buffer.from(regMsg, 'utf-8');
console.log('Buffer length:', buffer.length);

socket.send(buffer, 0, buffer.length, 8765, '127.0.0.1', (err) => {
  if (err) {
    console.error('❌ Send error:', err);
    process.exit(1);
  }
  console.log('✅ Sent successfully');
});

// Timeout
setTimeout(() => {
  console.log('⏱️  Timeout - no response');
  process.exit(1);
}, 3000);
