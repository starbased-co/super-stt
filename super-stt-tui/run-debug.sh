#!/bin/bash
# Run the TUI with debug output captured to a file

LOG_FILE="/tmp/super-stt-tui-debug.log"

echo "Starting Super STT TUI..."
echo "Debug output will be logged to: $LOG_FILE"
echo "Press 'q' to quit the TUI"
echo ""

pnpm start 2> "$LOG_FILE"

echo ""
echo "=== Debug Log ==="
cat "$LOG_FILE"
