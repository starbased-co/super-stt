#!/bin/bash
# Test raw UDP authentication

SECRET=$(cat /run/user/$(id -u)/stt/udp_secret)
MESSAGE="REGISTER:test:${SECRET}"

echo "Secret: $SECRET"
echo "Sending: $MESSAGE"
echo ""

# Send UDP packet and wait for response
echo "$MESSAGE" | nc -u -w 2 127.0.0.1 8765
