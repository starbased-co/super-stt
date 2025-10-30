# Transcription Display Debug Guide

## Debug Logging Added

The following logging has been added to trace transcription data flow:

### 1. Parser Level (`src/protocol/parsers.ts`)

**Partial STT** (Type 2 packets):
```
[parsePartialSTT] Parsed: { text, textLength, confidence, bufferLength }
```

**Final STT** (Type 3 packets):
```
[parseFinalSTT] Parsed: { text, textLength, confidence, bufferLength }
```

### 2. Event Emitter Level (`src/protocol/native-client.ts`)

**Partial STT events**:
```
[NativeUdpClient] Emitting partial_stt: { text, textLength, confidence }
```

**Final STT events**:
```
[NativeUdpClient] Emitting final_stt: { text, textLength, confidence }
```

### 3. Hook Level (`src/hooks/useUdpClient.ts`)

**Recording State Changes**:
```
[useUdpClient] Received recording_state event: { mounted, isRecording }
[useUdpClient] Setting recording state: { isRecording, partialTextCleared, finalTextPreserved, finalText }
```

**Partial STT Reception**:
```
[useUdpClient] Received partial_stt event: { mounted, text, textLength, confidence }
[useUdpClient] Setting partial state: { partialText, partialTextLength }
```

**Final STT Reception**:
```
[useUdpClient] Received final_stt event: { mounted, text, textLength, confidence }
[useUdpClient] Setting final state: { finalText, finalTextLength }
```

### 4. Component Level (`src/components/TranscriptionDisplay.tsx`)

**Every Render**:
```
[TranscriptionDisplay] Render: {
  partialText,
  partialTextLength,
  hasPartial,
  finalText,
  finalTextLength,
  hasFinal,
  partialConfidence,
  finalConfidence
}
```

## Testing Instructions

1. **Rebuild** (already done):
   ```bash
   pnpm build
   ```

2. **Run TUI**:
   ```bash
   pnpm start
   ```

3. **Start Recording**: Press 'r' to trigger recording

4. **Speak**: Say something into your microphone

5. **Watch Console Output**: Look for the logging chain above

## Expected Flow

When transcription is working correctly, you should see:

1. **While Speaking** (Partial transcription):
   ```
   [parsePartialSTT] Parsed: { text: "hello", textLength: 5, ... }
   [NativeUdpClient] Emitting partial_stt: { text: "hello", textLength: 5, ... }
   [useUdpClient] Received partial_stt event: { text: "hello", textLength: 5, ... }
   [useUdpClient] Setting partial state: { partialText: "hello", ... }
   [TranscriptionDisplay] Render: { partialText: "hello", hasPartial: true, ... }
   ```

2. **After Speaking** (Final transcription):
   ```
   [parseFinalSTT] Parsed: { text: "hello world", textLength: 11, ... }
   [NativeUdpClient] Emitting final_stt: { text: "hello world", textLength: 11, ... }
   [useUdpClient] Received final_stt event: { text: "hello world", textLength: 11, ... }
   [useUdpClient] Setting final state: { finalText: "hello world", ... }
   [TranscriptionDisplay] Render: { finalText: "hello world", hasFinal: true, ... }
   ```

## Potential Issues to Look For

### Issue 1: Events Not Emitted
**Symptom**: Parser logs appear but no `[NativeUdpClient]` logs
**Cause**: Event emitter not firing
**Location**: `src/protocol/native-client.ts` lines 119-134

### Issue 2: Events Not Received by Hook
**Symptom**: `[NativeUdpClient]` logs appear but no `[useUdpClient] Received` logs
**Cause**: Event listeners not registered or `mounted` is false
**Location**: `src/hooks/useUdpClient.ts` lines 134-179

### Issue 3: State Not Updating
**Symptom**: `[useUdpClient] Received` logs appear but no `[useUdpClient] Setting` logs
**Cause**: `mounted` flag is false or state update is blocked
**Location**: `src/hooks/useUdpClient.ts` lines 141-153, 164-178

### Issue 4: Component Not Re-rendering
**Symptom**: `[useUdpClient] Setting` logs appear but no new `[TranscriptionDisplay]` logs
**Cause**: React not detecting state change or component not receiving props
**Location**: App component prop passing or React rendering issue

### Issue 5: Text Cleared by Recording State
**Symptom**: Text appears briefly then disappears
**Cause**: `recording_state` event clearing text after it's set
**Location**: `src/hooks/useUdpClient.ts` lines 104-126
**Look for**: `finalTextPreserved: false` in recording state logs

### Issue 6: Empty String in Packets
**Symptom**: All logs show `textLength: 0` or empty strings
**Cause**: Daemon sending empty transcription or parser extracting empty strings
**Location**: Check `bufferLength` in parser logs - if buffer is minimal (15 bytes), no text data

## Debugging Commands

While TUI is running, check:

```bash
# Verify daemon is sending UDP packets
ss -ulnp | grep 8765

# Check TUI UDP socket
ss -ulnp | grep super-stt-tui

# Monitor daemon logs (if available)
journalctl -u super-stt-daemon -f
```

## Next Steps After Testing

Based on console output, we can determine:

1. **Where the data flow breaks** - Which log level stops appearing
2. **What the text content actually is** - Check `textLength` values
3. **Timing issues** - Look for recording_state events that clear text
4. **React rendering issues** - Component render frequency

Please run the TUI and share the console output showing the log sequence.
