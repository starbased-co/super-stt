/**
 * React hook for UDP client connection management
 *
 * Provides reactive state for all daemon data streams:
 * - Connection status
 * - Recording state
 * - Audio visualization data
 * - Transcription results
 */

import { useState, useEffect, useRef } from 'react';
import type { NativeUdpClient } from '../protocol/native-client.js';
import type {
  RecordingState,
  FrequencyBands,
  AudioSamples,
  STTResult,
} from '../protocol/types.js';

export interface UdpClientState {
  isConnected: boolean;
  isRegistered: boolean;
  isRecording: boolean;
  audioLevel: number;
  freqBands: number[];
  sampleRate: number;
  partialText: string;
  partialConfidence: number;
  finalText: string;
  finalConfidence: number;
  error: Error | null;
  clientId: string | null;
}

const INITIAL_STATE: UdpClientState = {
  isConnected: false,
  isRegistered: false,
  isRecording: false,
  audioLevel: 0,
  freqBands: [],
  sampleRate: 0,
  partialText: '',
  partialConfidence: 0,
  finalText: '',
  finalConfidence: 0,
  error: null,
  clientId: null,
};

/**
 * Hook for managing UDP client connection and state
 */
export function useUdpClient(): UdpClientState {
  const [state, setState] = useState<UdpClientState>(INITIAL_STATE);
  const clientRef = useRef<NativeUdpClient | null>(null);

  useEffect(() => {
    let mounted = true;

    async function connectClient() {
      try {
        // Create client without connecting yet
        const { NativeUdpClient } = await import('../protocol/native-client.js');
        const client = new NativeUdpClient();

        if (!mounted) {
          return;
        }

        clientRef.current = client;

      // Connection events - attach BEFORE connecting
      client.on('connected', () => {
        if (!mounted) return;
        setState((prev) => ({ ...prev, isConnected: true, error: null }));
      });

      client.on('disconnected', () => {
        if (!mounted) return;
        setState((prev) => ({
          ...prev,
          isConnected: false,
          isRegistered: false,
        }));
      });

      client.on('registered', (clientId) => {
        if (!mounted) return;
        setState((prev) => ({ ...prev, isRegistered: true, clientId }));
      });

      client.on('error', (error) => {
        if (!mounted) return;
        setState((prev) => ({ ...prev, error }));
      });

      // Recording state
      client.on('recording_state', (recordingState: RecordingState) => {
        if (!mounted) return;
        setState((prev) => ({
          ...prev,
          isRecording: recordingState.isRecording,
          // Clear partial text when recording stops
          partialText: recordingState.isRecording ? prev.partialText : '',
          partialConfidence: recordingState.isRecording ? prev.partialConfidence : 0,
        }));
      });

      // Frequency bands (primary visualization data)
      client.on('frequency_bands', (bands: FrequencyBands) => {
        if (!mounted) return;
        setState((prev) => ({
          ...prev,
          audioLevel: bands.totalEnergy,
          freqBands: bands.bands,
          sampleRate: bands.sampleRate,
        }));
      });

      // Audio samples (optional - for advanced visualizations)
      client.on('audio_samples', (_samples: AudioSamples) => {
        if (!mounted) return;
        // Could be used for custom DSP/visualization
        // For now, we primarily use frequency bands
      });

      // Partial transcription
      client.on('partial_stt', (result: STTResult) => {
        if (!mounted) return;
        setState((prev) => ({
          ...prev,
          partialText: result.text,
          partialConfidence: result.confidence,
        }));
      });

      // Final transcription
      client.on('final_stt', (result: STTResult) => {
        if (!mounted) return;
        setState((prev) => ({
          ...prev,
          finalText: result.text,
          finalConfidence: result.confidence,
          partialText: '',
          partialConfidence: 0,
        }));
      });

        // Now connect after all listeners are attached
        await client.connect('tui');

      } catch (error) {
        if (mounted) {
          setState((prev) => ({
            ...prev,
            error: error instanceof Error ? error : new Error(String(error)),
          }));
        }
      }
    }

    connectClient();

    // Cleanup on unmount
    return () => {
      mounted = false;
      if (clientRef.current) {
        clientRef.current.disconnect();
        clientRef.current = null;
      }
    };
  }, []);

  return state;
}
