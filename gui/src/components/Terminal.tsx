// Terminal component using xterm.js

import { onCleanup, onMount, createSignal, Show } from "solid-js";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebglAddon } from "@xterm/addon-webgl";
import { invoke } from "@tauri-apps/api/core";
import { terminalStore } from "../stores/terminalStore";
import { appStore } from "../stores/appStore";
import "@xterm/xterm/css/xterm.css";

interface TerminalProps {
  sessionId: string;
  sessionStatus?: string;
  onResize?: (rows: number, cols: number) => void;
}

export function Terminal(props: TerminalProps) {
  let containerRef: HTMLDivElement | undefined;
  let terminal: XTerm | undefined;
  let fitAddon: FitAddon | undefined;
  const [isStarting, setIsStarting] = createSignal(false);

  // Each Terminal instance is dedicated to a single session
  // Session switching is handled by showing/hiding Terminal containers in App.tsx
  const sessionId = props.sessionId;
  console.log(`[Terminal] MOUNT: sessionId=${sessionId}`);

  onMount(() => {
    if (!containerRef) return;

    // Create terminal instance
    // Use macOS default terminal fonts first - Menlo is the default in Terminal.app
    // SF Mono is used in newer macOS versions and has excellent Unicode support
    terminal = new XTerm({
      fontFamily: 'Menlo, "SF Mono", Monaco, "Courier New", monospace',
      fontSize: 14,
      theme: {
        background: "#1a1b26",
        foreground: "#c0caf5",
        cursor: "#c0caf5",
        cursorAccent: "#1a1b26",
        selectionBackground: "#33467c",
        black: "#15161e",
        brightBlack: "#414868",
        red: "#f7768e",
        brightRed: "#f7768e",
        green: "#9ece6a",
        brightGreen: "#9ece6a",
        yellow: "#e0af68",
        brightYellow: "#e0af68",
        blue: "#7aa2f7",
        brightBlue: "#7aa2f7",
        magenta: "#bb9af7",
        brightMagenta: "#bb9af7",
        cyan: "#7dcfff",
        brightCyan: "#7dcfff",
        white: "#a9b1d6",
        brightWhite: "#c0caf5",
      },
      cursorBlink: true,
      cursorStyle: "block",
      scrollback: 10000,
      allowProposedApi: true,
      // Disable scroll on output to prevent interference with alternate screen buffer
      scrollOnUserInput: true,
      // Don't convert line endings - let the PTY handle it
      convertEol: false,
      // Smoother rendering for TUI apps
      smoothScrollDuration: 0,
    });

    // Add addons
    fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.loadAddon(new WebLinksAddon());

    // Unicode11 addon for proper character width handling
    // Critical for TUI apps like Claude Code that use Unicode spinners, icons, etc.
    const unicode11 = new Unicode11Addon();
    terminal.loadAddon(unicode11);
    terminal.unicode.activeVersion = "11";

    // Open terminal in container
    terminal.open(containerRef);

    // Add WebGL addon for GPU-accelerated rendering (smoother updates)
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => {
        console.warn("[Terminal] WebGL context lost, disposing addon");
        webgl.dispose();
      });
      terminal.loadAddon(webgl);
      console.log("[Terminal] WebGL rendering enabled");
    } catch (e) {
      console.warn("[Terminal] WebGL not available, using canvas renderer:", e);
    }

    // Debug: Track scroll events to understand line pushing
    terminal.onScroll((scrollPos) => {
      console.log(`[Terminal] Scroll position: ${scrollPos}, buffer length: ${terminal?.buffer.active.length}`);
    });

    // Note: Don't fit or start session here - wait for ResizeObserver
    // The container may not have final dimensions yet during initial mount
    // ResizeObserver will fire when the container actually has layout dimensions

    // Handle input - send to daemon
    terminal.onData(async (data) => {
      try {
        // Base64 encode the input using proper UTF-8 handling
        const encoder = new TextEncoder();
        const bytes = encoder.encode(data);
        const binary = Array.from(bytes, (b) => String.fromCharCode(b)).join('');
        const encoded = btoa(binary);
        await invoke("send_input", {
          sessionId: sessionId,
          input: encoded,
        });
      } catch (e) {
        console.error("Failed to send input:", e);
      }
    });

    // Handle resize - debounce to prevent excessive resize calls
    let resizeTimeout: number | null = null;
    terminal.onResize(async ({ rows, cols }) => {
      props.onResize?.(rows, cols);

      // Debounce resize calls to PTY
      if (resizeTimeout) {
        clearTimeout(resizeTimeout);
      }
      resizeTimeout = window.setTimeout(async () => {
        console.log(`[Terminal] Resize: ${cols}x${rows} for session ${sessionId}`);
        try {
          await invoke("resize_session", {
            sessionId: sessionId,
            rows,
            cols,
          });
        } catch (e) {
          console.error("Failed to resize:", e);
        }
      }, 100);
    });

    // Handle container resize with debouncing and size change detection
    // Only call fit() when actual container dimensions change significantly
    let containerResizeTimeout: number | null = null;
    let lastWidth = 0;
    let lastHeight = 0;
    let hasInitialized = false; // Track if we've done initial setup
    const resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;

      const { width, height } = entry.contentRect;
      // Skip if container has no dimensions yet
      if (width === 0 || height === 0) return;

      // Only trigger resize if dimensions changed by more than 1px
      // This prevents spurious fit() calls from sub-pixel rendering changes
      if (Math.abs(width - lastWidth) > 1 || Math.abs(height - lastHeight) > 1) {
        lastWidth = width;
        lastHeight = height;

        if (containerResizeTimeout) {
          clearTimeout(containerResizeTimeout);
        }
        containerResizeTimeout = window.setTimeout(async () => {
          if (!fitAddon || !terminal) return;

          // Preserve scroll position across fit() calls
          const wasAtBottom = terminal.buffer.active.viewportY >= terminal.buffer.active.baseY;
          const previousViewportY = terminal.buffer.active.viewportY;

          fitAddon.fit();

          // Only auto-scroll to bottom if user was already at bottom
          // This preserves scroll position when user is reading scrollback
          if (!wasAtBottom && terminal.buffer.active.baseY > 0) {
            // User was scrolled up, restore approximate position
            terminal.scrollToLine(Math.min(previousViewportY, terminal.buffer.active.baseY));
          }
          const { rows, cols } = terminal;
          console.log(`[Terminal] Container resize: ${width}x${height}, terminal: ${cols}x${rows}`);

          // First resize with valid dimensions: initialize the session
          if (!hasInitialized) {
            hasInitialized = true;
            console.log(`[Terminal] Initial dimensions ready: ${cols}x${rows}, session ${sessionId} status: ${props.sessionStatus}`);

            if (props.sessionStatus === "stopped") {
              // Session is stopped - start it with correct dimensions
              setIsStarting(true);
              console.log(`[Terminal] Auto-starting stopped session ${sessionId} with size ${cols}x${rows}`);
              try {
                await appStore.restartSession(sessionId, rows, cols);
                console.log(`[Terminal] Session ${sessionId} started successfully`);
              } catch (e) {
                console.error("[Terminal] Failed to start session:", e);
              } finally {
                setIsStarting(false);
              }
            } else {
              // Session is already running - send resize to sync dimensions
              invoke("resize_session", {
                sessionId: sessionId,
                rows,
                cols,
              }).catch(console.error);
            }
          }
        }, 50);
      }
    });
    resizeObserver.observe(containerRef);

    // Register terminal instance with store (include fitAddon for dimension calculations)
    console.log(`[Terminal] REGISTER: sessionId=${sessionId}`);
    terminalStore.registerTerminal(sessionId, terminal, fitAddon);

    onCleanup(() => {
      console.log(`[Terminal] CLEANUP: sessionId=${sessionId}`);
      terminalStore.unregisterTerminal(sessionId);
      resizeObserver.disconnect();
      terminal?.dispose();
    });
  });

  // Each Terminal instance is dedicated to a single session (captured at mount time).
  // Session switching is handled by showing/hiding Terminal containers in App.tsx.

  return (
    <div class="relative w-full h-full">
      <div
        ref={containerRef}
        class="w-full h-full"
        style={{ background: "#1a1b26" }}
      />
      <Show when={isStarting()}>
        <div class="absolute inset-0 flex items-center justify-center bg-gray-900/80">
          <div class="text-center">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-400 mx-auto mb-2" />
            <p class="text-gray-300">Starting session...</p>
          </div>
        </div>
      </Show>
    </div>
  );
}

// Helper to decode base64 to proper UTF-8 string
// atob() alone doesn't handle multi-byte UTF-8 characters correctly
function base64ToUtf8(base64: string): string {
  const binaryString = atob(base64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return new TextDecoder("utf-8").decode(bytes);
}

// Helper to write output to a terminal instance
// This should be called from outside when we receive PTY output events
export function writeToTerminal(
  terminalRef: XTerm | undefined,
  data: string,
  isBase64: boolean = true
) {
  if (!terminalRef) return;

  try {
    const decoded = isBase64 ? base64ToUtf8(data) : data;
    terminalRef.write(decoded);
  } catch (e) {
    console.error("Failed to decode/write terminal data:", e);
    // Try writing raw data if decode fails
    terminalRef.write(data);
  }
}
