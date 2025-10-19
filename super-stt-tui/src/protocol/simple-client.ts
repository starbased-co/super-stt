/**
 * Simplified UDP client using the proven working pattern
 */

import dgram from 'node:dgram';
import { readFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { EventEmitter } from 'node:events';
import { parsePacket } from './parsers.js';
import type {
  RecordingState,
  FrequencyBands,
  AudioSamples,
  STTResult,
} from './types.js';

const DAEMON_HOST = '127.0.0.1';
const DAEMON_PORT = 8765;
const KEEP_ALIVE_INTERVAL = 60_000;

export interface UdpClientEvents {
  connected: [];
  disconnected: [];
  error: [error: Error];
  registered: [clientId: string];
  recording_state: [state: RecordingState];
  frequency_bands: [bands: FrequencyBands];
  audio_samples: [samples: AudioSamples];
  partial_stt: [result: STTResult];
  final_stt: [result: STTResult];
}

export declare interface SimpleUdpClient {
  on<K extends keyof UdpClientEvents>(
    event: K,
    listener: (...args: UdpClientEvents[K]) => void,
  ): this;
  emit<K extends keyof UdpClientEvents>(event: K, ...args: UdpClientEvents[K]): boolean;
}

export class SimpleUdpClient extends EventEmitter {
  private socket: dgram.Socket | null = null;
  private keepAliveTimer: NodeJS.Timeout | null = null;

  private getOrCreateSecret(): string {
    const runtimeDir = process.env.XDG_RUNTIME_DIR || process.env.TMPDIR || '/tmp';
    const secretDir = join(runtimeDir, 'stt');
    const secretPath = join(secretDir, 'udp_secret');

    if (existsSync(secretPath)) {
      return readFileSync(secretPath, 'utf-8').trim();
    }

    if (!existsSync(secretDir)) {
      mkdirSync(secretDir, { recursive: true, mode: 0o700 });
    }

    const timestamp = Date.now() * 1000000 + process.hrtime()[1];
    const pid = process.pid;
    const secret = `stt_${timestamp}_${pid}`;

    writeFileSync(secretPath, secret, { mode: 0o600 });
    return secret;
  }

  connect(clientType = 'tui'): void {
    if (this.socket) return;

    try {
      console.error('[UDP] Starting connection...');
      const socket = dgram.createSocket('udp4');
      console.error('[UDP] Socket created');

      const secret = this.getOrCreateSecret();
      console.error('[UDP] Secret obtained:', secret.substring(0, 20));

    socket.on('message', (msg) => {
      const data = msg.toString();

      if (data.startsWith('REGISTERED:')) {
        const clientId = data.split(':')[1];
        console.log(`✅ Registered: ${clientId}`);

        // Start keep-alive
        this.keepAliveTimer = setInterval(() => {
          const pingBuffer = Buffer.from('PING', 'utf-8');
          socket.send(pingBuffer, 0, pingBuffer.length, DAEMON_PORT, DAEMON_HOST);
        }, KEEP_ALIVE_INTERVAL);

        this.emit('registered', clientId);
        return;
      }

      if (data === 'PONG') return;

      // Parse binary packets
      const parsed = parsePacket(msg);
      if (!parsed) return;

      switch (parsed.type) {
        case 'recording_state':
          this.emit('recording_state', parsed.data);
          break;
        case 'frequency_bands':
          this.emit('frequency_bands', parsed.data);
          break;
        case 'audio_samples':
          this.emit('audio_samples', parsed.data);
          break;
        case 'partial_stt':
          this.emit('partial_stt', parsed.data);
          break;
        case 'final_stt':
          this.emit('final_stt', parsed.data);
          break;
      }
    });

    socket.on('error', (error) => {
      console.error('❌ Socket error:', error);
      this.emit('error', error);
    });

    // Send registration
    const regMsg = `REGISTER:${clientType}:${secret}`;
    const buffer = Buffer.from(regMsg, 'utf-8');

    console.error('[UDP] Sending buffer of length:', buffer.length);
    socket.send(buffer, 0, buffer.length, DAEMON_PORT, DAEMON_HOST, (err) => {
      if (err) {
        console.error('[UDP] ❌ Send failed:', err);
        this.emit('error', err);
        return;
      }
      console.error('[UDP] ✅ Registration sent');
    });

      this.socket = socket;
      this.emit('connected');
    } catch (error) {
      console.error('[UDP] FATAL ERROR:', error);
      this.emit('error', error instanceof Error ? error : new Error(String(error)));
    }
  }

  disconnect(): void {
    if (this.keepAliveTimer) {
      clearInterval(this.keepAliveTimer);
      this.keepAliveTimer = null;
    }

    if (this.socket) {
      this.socket.close();
      this.socket = null;
    }

    this.emit('disconnected');
  }
}

export function createSimpleUdpClient(clientType = 'tui'): SimpleUdpClient {
  const client = new SimpleUdpClient();
  client.connect(clientType);
  return client;
}
