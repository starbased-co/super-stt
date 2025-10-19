#!/usr/bin/env node
import { render } from 'ink';
import { App } from './app.js';

/**
 * Super STT TUI Entry Point
 *
 * This application provides a terminal interface for the Super STT daemon,
 * displaying real-time audio visualizations and speech-to-text transcriptions.
 */

// Clear the console before starting
console.clear();

// Initialize the Ink app
const { unmount } = render(<App />);

// Handle graceful shutdown
const shutdown = () => {
  unmount();
  console.log('\nSuper STT TUI shut down gracefully.');
  process.exit(0);
};

// Register signal handlers
process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);

// Handle uncaught errors
process.on('uncaughtException', (error) => {
  console.error('Uncaught exception:', error);
  shutdown();
});

process.on('unhandledRejection', (reason, promise) => {
  console.error('Unhandled rejection at:', promise, 'reason:', reason);
  shutdown();
});