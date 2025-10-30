/**
 * Native UDP client wrapper using Rust N-API binding
 * Provides event-based interface matching the existing client pattern
 */

import { EventEmitter } from 'node:events';
import { createRequire } from 'node:module';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parsePacket } from './parsers.js';
import type {
  RecordingState,
  FrequencyBands,
  AudioSamples,
  STTResult,
} from './types.js';

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));
const nativeModulePath = join(__dirname, '../../super-stt-tui-native.linux-x64-gnu.node');
const { UdpClient } = require(nativeModulePath);

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

export declare interface NativeUdpClient {
  on<K extends keyof UdpClientEvents>(
    event: K,
    listener: (...args: UdpClientEvents[K]) => void,
  ): this;
  emit<K extends keyof UdpClientEvents>(event: K, ...args: UdpClientEvents[K]): boolean;
}

/**
 * Native UDP client wrapper with event emitter interface
 */
export class NativeUdpClient extends EventEmitter {
  private client: any = null;
  private keepAliveTimer: NodeJS.Timeout | null = null;
  private receiveLoopRunning = false;
  private shouldStop = false;

  async connect(clientType = 'tui'): Promise<void> {
    if (this.client) {
      throw new Error('Client already connected');
    }

    try {
      this.client = new UdpClient();
      const response = await this.client.connect(clientType);

      if (response.startsWith('REGISTERED:')) {
        const clientId = response.split(':')[1];
        this.emit('connected');
        this.emit('registered', clientId);

        this.startKeepAlive();
        this.startReceiveLoop();
      } else {
        throw new Error(`Unexpected response: ${response}`);
      }
    } catch (error) {
      this.emit('error', error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  private startKeepAlive(): void {
    this.keepAliveTimer = setInterval(async () => {
      if (!this.client) return;

      try {
        await this.client.sendPing();
      } catch (error) {
        this.emit('error', error instanceof Error ? error : new Error(String(error)));
      }
    }, KEEP_ALIVE_INTERVAL);
  }

  private async startReceiveLoop(): Promise<void> {
    if (this.receiveLoopRunning) return;

    this.receiveLoopRunning = true;
    this.shouldStop = false;

    while (!this.shouldStop && this.client) {
      try {
        const buffer = await this.client.receivePacket();

        if (buffer.length === 4) {
          const text = buffer.toString('utf-8');
          if (text === 'PONG') continue;
        }

        const parsed = parsePacket(buffer);
        if (!parsed) continue;

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
            console.log('[NativeUdpClient] Emitting partial_stt:', {
              text: parsed.data.text,
              textLength: parsed.data.text.length,
              confidence: parsed.data.confidence,
            });
            this.emit('partial_stt', parsed.data);
            break;
          case 'final_stt':
            console.log('[NativeUdpClient] Emitting final_stt:', {
              text: parsed.data.text,
              textLength: parsed.data.text.length,
              confidence: parsed.data.confidence,
            });
            this.emit('final_stt', parsed.data);
            break;
        }
      } catch (error) {
        if (!this.shouldStop) {
          this.emit('error', error instanceof Error ? error : new Error(String(error)));
        }
        break;
      }
    }

    this.receiveLoopRunning = false;
  }

  disconnect(): void {
    this.shouldStop = true;

    if (this.keepAliveTimer) {
      clearInterval(this.keepAliveTimer);
      this.keepAliveTimer = null;
    }

    if (this.client) {
      try {
        this.client.disconnect();
      } catch (error) {
        console.error('Error during disconnect:', error);
      }
      this.client = null;
    }

    this.emit('disconnected');
  }

  isConnected(): boolean {
    return this.client ? this.client.isConnected() : false;
  }

  getClientId(): string | null {
    return this.client ? this.client.getClientId() : null;
  }

  async sendRecordCommand(socketPath?: string, writeMode = true): Promise<string> {
    if (!this.client) {
      throw new Error('Client not connected');
    }
    return await this.client.sendRecordCommand(socketPath, writeMode);
  }
}

/**
 * Factory function to create and connect a native UDP client
 */
export async function createNativeUdpClient(clientType = 'tui'): Promise<NativeUdpClient> {
  const client = new NativeUdpClient();
  await client.connect(clientType);
  return client;
}
