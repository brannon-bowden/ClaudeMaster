// Terminal component using xterm.js

import { createEffect, onCleanup, onMount } from "solid-js";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { invoke } from "@tauri-apps/api/core";
import { terminalStore } from "../stores/terminalStore";
import "@xterm/xterm/css/xterm.css";

interface TerminalProps {
  sessionId: string;
  onResize?: (rows: number, cols: number) => void;
}

export function Terminal(props: TerminalProps) {
  let containerRef: HTMLDivElement | undefined;
  let terminal: XTerm | undefined;
  let fitAddon: FitAddon | undefined;

  onMount(() => {
    if (!containerRef) return;

    // Create terminal instance
    terminal = new XTerm({
      fontFamily: '"JetBrains Mono", "Fira Code", monospace',
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
    });

    // Add addons
    fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.loadAddon(new WebLinksAddon());

    // Open terminal in container
    terminal.open(containerRef);
    fitAddon.fit();

    // Handle input - send to daemon
    terminal.onData(async (data) => {
      try {
        // Base64 encode the input
        const encoded = btoa(data);
        await invoke("send_input", {
          sessionId: props.sessionId,
          input: encoded,
        });
      } catch (e) {
        console.error("Failed to send input:", e);
      }
    });

    // Handle resize
    terminal.onResize(async ({ rows, cols }) => {
      props.onResize?.(rows, cols);
      try {
        await invoke("resize_session", {
          sessionId: props.sessionId,
          rows,
          cols,
        });
      } catch (e) {
        console.error("Failed to resize:", e);
      }
    });

    // Initial resize notification
    const { rows, cols } = terminal;
    invoke("resize_session", {
      sessionId: props.sessionId,
      rows,
      cols,
    }).catch(console.error);

    // Handle container resize
    const resizeObserver = new ResizeObserver(() => {
      fitAddon?.fit();
    });
    resizeObserver.observe(containerRef);

    // Register terminal instance with store
    terminalStore.registerTerminal(props.sessionId, terminal);

    onCleanup(() => {
      terminalStore.unregisterTerminal(props.sessionId);
      resizeObserver.disconnect();
      terminal?.dispose();
    });
  });

  // Re-fit when session changes
  createEffect(() => {
    // Track session ID as dependency
    void props.sessionId;
    fitAddon?.fit();
  });

  return (
    <div
      ref={containerRef}
      class="w-full h-full"
      style={{ background: "#1a1b26" }}
    />
  );
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
    const decoded = isBase64 ? atob(data) : data;
    terminalRef.write(decoded);
  } catch (e) {
    console.error("Failed to decode/write terminal data:", e);
    // Try writing raw data if decode fails
    terminalRef.write(data);
  }
}
