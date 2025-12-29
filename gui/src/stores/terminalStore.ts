// Terminal output store - manages PTY output buffers

import { createSignal } from "solid-js";
import { Terminal as XTerm } from "@xterm/xterm";

// Map of session ID to terminal instance
const terminalInstances = new Map<string, XTerm>();

// Map of session ID to buffered output (before terminal is mounted)
const outputBuffers = new Map<string, string[]>();

// Signal for forcing terminal updates
const [terminalUpdateTrigger, setTerminalUpdateTrigger] = createSignal(0);

// Register a terminal instance for a session
export function registerTerminal(sessionId: string, terminal: XTerm) {
  terminalInstances.set(sessionId, terminal);

  // Flush any buffered output
  const buffer = outputBuffers.get(sessionId);
  if (buffer && buffer.length > 0) {
    for (const data of buffer) {
      writeBase64ToTerminal(sessionId, data);
    }
    outputBuffers.delete(sessionId);
  }
}

// Unregister a terminal instance
export function unregisterTerminal(sessionId: string) {
  terminalInstances.delete(sessionId);
}

// Get terminal instance for a session
export function getTerminal(sessionId: string): XTerm | undefined {
  return terminalInstances.get(sessionId);
}

// Write base64-encoded data to a terminal
export function writeBase64ToTerminal(sessionId: string, base64Data: string) {
  const terminal = terminalInstances.get(sessionId);

  if (terminal) {
    try {
      const decoded = atob(base64Data);
      terminal.write(decoded);
    } catch (e) {
      console.error("Failed to decode terminal data:", e);
      terminal.write(base64Data);
    }
  } else {
    // Buffer the output until terminal is mounted
    const buffer = outputBuffers.get(sessionId) || [];
    buffer.push(base64Data);
    outputBuffers.set(sessionId, buffer);

    // Limit buffer size to prevent memory issues
    if (buffer.length > 1000) {
      buffer.splice(0, buffer.length - 1000);
    }
  }

  // Trigger update
  setTerminalUpdateTrigger((n) => n + 1);
}

// Clear buffered output for a session
export function clearBuffer(sessionId: string) {
  outputBuffers.delete(sessionId);
}

// Clear all buffers
export function clearAllBuffers() {
  outputBuffers.clear();
}

export const terminalStore = {
  registerTerminal,
  unregisterTerminal,
  getTerminal,
  writeBase64ToTerminal,
  clearBuffer,
  clearAllBuffers,
  terminalUpdateTrigger,
};
