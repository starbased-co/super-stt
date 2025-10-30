/**
 * UDP client manager for Super STT daemon communication
 *
 * Handles:
 * - Authentication via shared secret
 * - Connection registration
 * - Keep-alive pings
 * - Packet reception and parsing
 * - Event emission for parsed data
 */

import dgram from "node:dgram";
import { readFileSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { EventEmitter } from "node:events";
import { parsePacket } from "./parsers.js";
import type {
  RecordingState,
  FrequencyBands,
  AudioSamples,
  STTResult,
} from "./types.js";

const DAEMON_HOST = "127.0.0.1";
const DAEMON_PORT = 8765;
const KEEP_ALIVE_INTERVAL = 60_000; // 60 seconds
const REGISTRATION_TIMEOUT = 5_000; // 5 seconds

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

export declare interface UdpClient {
  on<K extends keyof UdpClientEvents>(
    event: K,
    listener: (...args: UdpClientEvents[K]) => void,
  ): this;
  emit<K extends keyof UdpClientEvents>(
    event: K,
    ...args: UdpClientEvents[K]
  ): boolean;
}

export class UdpClient extends EventEmitter {
  private socket: dgram.Socket | null = null;
  private keepAliveTimer: NodeJS.Timeout | null = null;
  private isRegistered = false;
  private registrationTimeout: NodeJS.Timeout | null = null;

  /**
   * Get or create shared secret for authentication
   * Follows the same lazy initialization pattern as the Rust daemon
   */
  private getOrCreateSecret(): string {
    const runtimeDir =
      process.env.XDG_RUNTIME_DIR || process.env.TMPDIR || "/tmp";
    const secretDir = join(runtimeDir, "stt");
    const secretPath = join(secretDir, "udp_secret");
    console.log(secretPath);

    // If secret file exists, load it
    if (existsSync(secretPath)) {
      try {
        return readFileSync(secretPath, "utf-8").trim();
      } catch (error) {
        throw new Error(
          `Failed to read secret file: ${error instanceof Error ? error.message : String(error)}`,
        );
      }
    }

    // Create directory if it doesn't exist
    if (!existsSync(secretDir)) {
      mkdirSync(secretDir, { recursive: true, mode: 0o700 });
    }

    // Generate new secret using timestamp and PID
    const timestamp = Date.now() * 1000000 + process.hrtime()[1];
    const pid = process.pid;
    const secret = `stt_${timestamp}_${pid}`;

    // Write secret file with restrictive permissions
    try {
      writeFileSync(secretPath, secret, { mode: 0o600 });
      return secret;
    } catch (error) {
      throw new Error(
        `Failed to create secret file: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  }

  /**
   * Connect to daemon and register
   */
  connect(clientType = "tui"): void {
    if (this.socket) {
      console.warn("Already connected");
      return;
    }

    try {
      const secret = this.getOrCreateSecret();
      console.log("📡 Connecting to daemon...");
      console.log(`   Secret: ${secret.substring(0, 15)}...`);
      console.log("secret", secret);

      this.socket = dgram.createSocket("udp4");

      this.socket.on("message", this.handleMessage.bind(this));
      this.socket.on("error", this.handleError.bind(this));
      this.socket.on("close", this.handleClose.bind(this));

      // Send registration message
      const registrationMsg = `REGISTER:${clientType}:${secret}`;
      console.log(
        `   Sending: REGISTER:${clientType}:${secret.substring(0, 15)}...`,
      );

      const msgBuffer = Buffer.from(registrationMsg, "utf-8");
      console.log(
        `   Buffer length: ${msgBuffer.length}, content: ${msgBuffer.toString()}`,
      );

      this.socket.send(
        msgBuffer,
        0,
        msgBuffer.length,
        DAEMON_PORT,
        DAEMON_HOST,
        (err) => {
          if (err) {
            console.error("❌ Registration send failed:", err.message);
            this.emit(
              "error",
              new Error(`Registration failed: ${err.message}`),
            );
            this.disconnect();
            return;
          }
          console.log("✅ Registration message sent to daemon");
        },
      );

      // Set registration timeout
      this.registrationTimeout = setTimeout(() => {
        if (!this.isRegistered) {
          console.warn("⚠️  Registration timeout - no response from daemon");
          this.emit(
            "error",
            new Error("Registration timeout - daemon not responding"),
          );
          this.disconnect();
        }
      }, REGISTRATION_TIMEOUT);

      this.emit("connected");
    } catch (error) {
      console.error("❌ Connection error:", error);
      this.emit(
        "error",
        error instanceof Error ? error : new Error(String(error)),
      );
    }
  }

  /**
   * Disconnect from daemon
   */
  disconnect(): void {
    if (this.keepAliveTimer) {
      clearInterval(this.keepAliveTimer);
      this.keepAliveTimer = null;
    }

    if (this.registrationTimeout) {
      clearTimeout(this.registrationTimeout);
      this.registrationTimeout = null;
    }

    if (this.socket) {
      this.socket.close();
      this.socket = null;
    }

    this.isRegistered = false;
    this.emit("disconnected");
  }

  /**
   * Send keep-alive ping
   */
  private sendPing(): void {
    if (!this.socket || !this.isRegistered) return;

    const pingBuffer = Buffer.from("PING", "utf-8");
    this.socket.send(
      pingBuffer,
      0,
      pingBuffer.length,
      DAEMON_PORT,
      DAEMON_HOST,
      (err) => {
        if (err) {
          console.error("Keep-alive ping failed:", err);
          this.emit("error", new Error(`Keep-alive failed: ${err.message}`));
        }
      },
    );
  }

  /**
   * Handle incoming messages
   */
  private handleMessage(msg: Buffer): void {
    const data = msg.toString();

    // Handle registration acknowledgment
    if (data.startsWith("REGISTERED:")) {
      const clientId = data.split(":")[1];
      this.isRegistered = true;
      console.log(`✅ Registered with daemon: ${clientId}`);

      if (this.registrationTimeout) {
        clearTimeout(this.registrationTimeout);
        this.registrationTimeout = null;
      }

      // Start keep-alive pings
      this.keepAliveTimer = setInterval(() => {
        this.sendPing();
      }, KEEP_ALIVE_INTERVAL);

      this.emit("registered", clientId);
      return;
    }

    // Handle PONG responses (ignore)
    if (data === "PONG") {
      return;
    }

    // Parse binary packets
    const parsed = parsePacket(msg);
    if (!parsed) return;

    switch (parsed.type) {
      case "recording_state":
        console.log("📝 Recording state");
        this.emit("recording_state", parsed.data);
        break;
      case "frequency_bands":
        console.log("📊 Frequency bands");
        this.emit("frequency_bands", parsed.data);
        break;
      case "audio_samples":
        console.log("🎵 Audio samples");
        this.emit("audio_samples", parsed.data);
        break;
      case "partial_stt":
        console.log("💬 Partial STT");
        this.emit("partial_stt", parsed.data);
        break;
      case "final_stt":
        console.log("✨ Final STT");
        this.emit("final_stt", parsed.data);
        break;
      case "unknown":
        console.warn("⚠️  Unknown packet");
        break;
    }
  }

  /**
   * Handle socket errors
   */
  private handleError(error: Error): void {
    console.error("UDP socket error:", error);
    this.emit("error", error);
  }

  /**
   * Handle socket close
   */
  private handleClose(): void {
    this.disconnect();
  }

  /**
   * Check if client is connected and registered
   */
  get connected(): boolean {
    return this.socket !== null && this.isRegistered;
  }
}

/**
 * Create and connect a new UDP client
 */
export function createUdpClient(clientType = "tui"): UdpClient {
  const client = new UdpClient();
  client.connect(clientType);
  return client;
}
