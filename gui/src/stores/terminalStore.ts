// Terminal output store - manages PTY output buffers

import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";

// Map of session ID to terminal instance and FitAddon
interface TerminalEntry {
  terminal: XTerm;
  fitAddon: FitAddon;
}
const terminalInstances = new Map<string, TerminalEntry>();

// Map of session ID to buffered output (before terminal is mounted)
const outputBuffers = new Map<string, string[]>();

// Register a terminal instance for a session
export function registerTerminal(sessionId: string, terminal: XTerm, fitAddon: FitAddon) {
  const existingIds = Array.from(terminalInstances.keys());
  console.log(`[TerminalStore] REGISTER: ${sessionId}, existing terminals: [${existingIds.join(', ')}]`);

  terminalInstances.set(sessionId, { terminal, fitAddon });

  // Flush any buffered output
  const buffer = outputBuffers.get(sessionId);
  if (buffer && buffer.length > 0) {
    console.log(`[TerminalStore] Flushing ${buffer.length} buffered items for ${sessionId}`);
    for (const data of buffer) {
      writeBase64ToTerminal(sessionId, data);
    }
    outputBuffers.delete(sessionId);
  }
}

// Unregister a terminal instance
export function unregisterTerminal(sessionId: string) {
  const wasRegistered = terminalInstances.has(sessionId);
  terminalInstances.delete(sessionId);
  const remainingIds = Array.from(terminalInstances.keys());
  console.log(`[TerminalStore] UNREGISTER: ${sessionId} (was registered: ${wasRegistered}), remaining: [${remainingIds.join(', ')}]`);
}

// Get terminal instance for a session
export function getTerminal(sessionId: string): XTerm | undefined {
  return terminalInstances.get(sessionId)?.terminal;
}

// Streaming UTF-8 decoders per session - handles multi-byte chars split across reads
const streamingDecoders = new Map<string, TextDecoder>();

// Get or create a streaming decoder for a session
function getStreamingDecoder(sessionId: string): TextDecoder {
  let decoder = streamingDecoders.get(sessionId);
  if (!decoder) {
    // Create decoder with stream=true to handle incomplete multi-byte sequences
    decoder = new TextDecoder("utf-8", { fatal: false });
    streamingDecoders.set(sessionId, decoder);
  }
  return decoder;
}

// Reset decoder for a session (call when session restarts)
export function resetDecoder(sessionId: string) {
  streamingDecoders.delete(sessionId);
}

// Helper to decode base64 to proper UTF-8 string with streaming support
// Uses streaming decoder to handle multi-byte UTF-8 characters split across PTY reads
function base64ToUtf8Streaming(sessionId: string, base64: string): string {
  const binaryString = atob(base64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  // Use streaming decoder - it buffers incomplete multi-byte sequences
  const decoder = getStreamingDecoder(sessionId);
  return decoder.decode(bytes, { stream: true });
}

// Debug: count writes and track escape sequences
let writeCount = 0;
let lastLogTime = Date.now();
let inAlternateScreen = false;

// Write base64-encoded data to a terminal
export function writeBase64ToTerminal(sessionId: string, base64Data: string) {
  const entry = terminalInstances.get(sessionId);

  if (entry) {
    try {
      // Use streaming decoder to handle multi-byte UTF-8 chars split across reads
      const decoded = base64ToUtf8Streaming(sessionId, base64Data);

      // Track alternate screen buffer mode
      // Enter: \x1b[?1049h or \x1b[?47h or \x1b[?1047h
      // Exit: \x1b[?1049l or \x1b[?47l or \x1b[?1047l
      if (/\x1b\[\?(?:1049|47|1047)h/.test(decoded)) {
        console.log('[TerminalStore] ENTERED alternate screen buffer');
        inAlternateScreen = true;
      }
      if (/\x1b\[\?(?:1049|47|1047)l/.test(decoded)) {
        console.log('[TerminalStore] EXITED alternate screen buffer');
        inAlternateScreen = false;
      }

      // Debug logging - detailed escape sequence analysis for first 50 writes, then periodic
      writeCount++;
      const now = Date.now();
      const shouldLog = writeCount <= 50 || writeCount % 100 === 0 || now - lastLogTime > 2000;

      if (shouldLog) {
        // Analyze escape sequences in detail
        const newlineCount = (decoded.match(/\n/g) || []).length;
        const crCount = (decoded.match(/\r/g) || []).length;
        const cursorUp = (decoded.match(/\x1b\[(\d*)A/g) || []).length;
        const cursorDown = (decoded.match(/\x1b\[(\d*)B/g) || []).length;
        const cursorForward = (decoded.match(/\x1b\[(\d*)C/g) || []).length;
        const cursorBack = (decoded.match(/\x1b\[(\d*)D/g) || []).length;
        const clearLine = (decoded.match(/\x1b\[(\d*)K/g) || []).length;
        const clearScreen = (decoded.match(/\x1b\[(\d*)J/g) || []).length;
        const scrollUp = decoded.includes('\x1b[S');
        const scrollDown = decoded.includes('\x1b[T');
        const cursorPos = (decoded.match(/\x1b\[(\d+);(\d+)H/g) || []).length;
        const saveCursor = decoded.includes('\x1b[s') || decoded.includes('\x1b7');
        const restoreCursor = decoded.includes('\x1b[u') || decoded.includes('\x1b8');

        console.log(`[TerminalStore] Write #${writeCount}: ${decoded.length}B | LF:${newlineCount} CR:${crCount} | ↑:${cursorUp} ↓:${cursorDown} →:${cursorForward} ←:${cursorBack} | clrLn:${clearLine} clrScr:${clearScreen} | pos:${cursorPos} save:${saveCursor} restore:${restoreCursor} | scroll↑:${scrollUp} scroll↓:${scrollDown} | altScr:${inAlternateScreen}`);

        // For first 10 writes, also show hex dump of escape sequences
        if (writeCount <= 10) {
          const escapeSeqs = decoded.match(/\x1b[^\x1b]{1,20}/g) || [];
          if (escapeSeqs.length > 0) {
            console.log(`[TerminalStore] Escape sequences in write #${writeCount}:`, escapeSeqs.map(s =>
              s.split('').map(c => c.charCodeAt(0) < 32 || c.charCodeAt(0) > 126 ? `\\x${c.charCodeAt(0).toString(16).padStart(2, '0')}` : c).join('')
            ).slice(0, 10));
          }
        }

        // Log Unicode characters (code points > 127) that might render as "?"
        const unicodeChars = decoded.match(/[\u0080-\uFFFF]/g) || [];
        if (unicodeChars.length > 0) {
          const uniqueChars = [...new Set(unicodeChars)];
          console.log(`[TerminalStore] Unicode chars in write #${writeCount}:`, uniqueChars.map(c =>
            `${c} (U+${c.codePointAt(0)?.toString(16).toUpperCase().padStart(4, '0')})`
          ).slice(0, 20));
        }
        lastLogTime = now;
      }

      entry.terminal.write(decoded);
    } catch (e) {
      console.error("Failed to decode terminal data:", e);
      entry.terminal.write(base64Data);
    }
  } else {
    // Buffer the output until terminal is mounted
    const buffer = outputBuffers.get(sessionId) || [];
    buffer.push(base64Data);
    outputBuffers.set(sessionId, buffer);

    // Log when buffering - this helps debug session switching issues
    if (buffer.length <= 5 || buffer.length % 100 === 0) {
      const registeredIds = Array.from(terminalInstances.keys());
      console.log(`[TerminalStore] BUFFERING for ${sessionId} (buffer size: ${buffer.length}), registered terminals: [${registeredIds.join(', ')}]`);
    }

    // Limit buffer size to prevent memory issues
    if (buffer.length > 1000) {
      buffer.splice(0, buffer.length - 1000);
    }
  }
}

// Clear buffered output for a session
export function clearBuffer(sessionId: string) {
  outputBuffers.delete(sessionId);
}

// Clear all buffers
export function clearAllBuffers() {
  outputBuffers.clear();
}

// Clear terminal screen for a session
export function clearTerminal(sessionId: string) {
  const entry = terminalInstances.get(sessionId);
  if (entry) {
    entry.terminal.clear();
  }
  // Also clear any buffered output and reset decoder
  outputBuffers.delete(sessionId);
  resetDecoder(sessionId);
}

// Get terminal dimensions for a session
// Forces a re-fit to ensure dimensions are accurate for current container size
// Returns { rows, cols } or null if terminal not found
export function getTerminalDimensions(sessionId: string): { rows: number; cols: number } | null {
  const entry = terminalInstances.get(sessionId);
  if (entry) {
    // Force re-fit to get accurate dimensions based on current container size
    // This prevents stale dimensions when container has resized
    try {
      entry.fitAddon.fit();
    } catch (e) {
      console.warn('[TerminalStore] Failed to fit terminal:', e);
    }
    const { rows, cols } = entry.terminal;
    console.log(`[TerminalStore] getTerminalDimensions after fit: ${cols}x${rows}`);
    return { rows, cols };
  }
  return null;
}

export const terminalStore = {
  registerTerminal,
  unregisterTerminal,
  getTerminal,
  writeBase64ToTerminal,
  clearBuffer,
  clearAllBuffers,
  clearTerminal,
  getTerminalDimensions,
  resetDecoder,
};
