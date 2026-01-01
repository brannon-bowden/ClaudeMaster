// Keyboard shortcuts hook for Claude Master
// Uses Cmd on Mac, Ctrl elsewhere

import { onMount, onCleanup } from "solid-js";
import { appStore } from "../stores/appStore";
import { terminalStore } from "../stores/terminalStore";

// Detect if we're on Mac
const isMac = navigator.platform.toUpperCase().indexOf("MAC") >= 0;

// Modifier key based on platform
const modKey = isMac ? "metaKey" : "ctrlKey";

export interface KeyboardShortcutHandlers {
  onNewSession?: () => void;
  onNewGroup?: () => void;
  onSettings?: () => void;
  onFocusSearch?: () => void;
}

export function useKeyboardShortcuts(handlers: KeyboardShortcutHandlers = {}) {
  const handleKeyDown = (e: KeyboardEvent) => {
    // Check if modifier key is pressed
    if (!e[modKey]) return;

    // Ignore if typing in an input field (except for global shortcuts)
    const target = e.target as HTMLElement;
    const isInput =
      target.tagName === "INPUT" ||
      target.tagName === "TEXTAREA" ||
      target.isContentEditable;

    switch (e.key.toLowerCase()) {
      // Cmd/Ctrl + N: New session, Cmd/Ctrl + Shift + N: New group
      case "n":
        if (!isInput) {
          e.preventDefault();
          if (e.shiftKey) {
            handlers.onNewGroup?.();
          } else {
            handlers.onNewSession?.();
          }
        }
        break;

      // Cmd/Ctrl + K: Focus search
      case "k":
        e.preventDefault();
        handlers.onFocusSearch?.();
        break;

      // Cmd/Ctrl + ,: Open settings
      case ",":
        if (!isInput) {
          e.preventDefault();
          handlers.onSettings?.();
        }
        break;

      // Cmd/Ctrl + Enter: Send input to selected session
      case "enter":
        // This is handled in the terminal component
        break;

      // Cmd/Ctrl + W: Stop selected session
      case "w":
        if (!isInput) {
          e.preventDefault();
          const sessionId = appStore.selectedSessionId();
          if (sessionId) {
            appStore.stopSession(sessionId).catch(console.error);
          }
        }
        break;

      // Cmd/Ctrl + R: Restart selected session
      case "r":
        if (!isInput) {
          e.preventDefault();
          const sessionId = appStore.selectedSessionId();
          if (sessionId) {
            appStore.restartSession(sessionId).catch(console.error);
          }
        }
        break;

      // Cmd/Ctrl + L: Clear terminal (handled in terminal)
      case "l":
        if (!isInput) {
          e.preventDefault();
          const sessionId = appStore.selectedSessionId();
          if (sessionId) {
            terminalStore.clearTerminal(sessionId);
          }
        }
        break;

      // Cmd/Ctrl + 1-9: Select session by index
      case "1":
      case "2":
      case "3":
      case "4":
      case "5":
      case "6":
      case "7":
      case "8":
      case "9":
        if (!isInput) {
          e.preventDefault();
          const index = parseInt(e.key) - 1;
          const sessions = appStore.sessions();
          if (index < sessions.length) {
            appStore.setSelectedSessionId(sessions[index].id);
          }
        }
        break;

      // Cmd/Ctrl + ]: Next session
      case "]":
        if (!isInput) {
          e.preventDefault();
          const sessions = appStore.sessions();
          const currentId = appStore.selectedSessionId();
          if (sessions.length > 0) {
            const currentIndex = sessions.findIndex((s) => s.id === currentId);
            const nextIndex =
              currentIndex === -1 ? 0 : (currentIndex + 1) % sessions.length;
            appStore.setSelectedSessionId(sessions[nextIndex].id);
          }
        }
        break;

      // Cmd/Ctrl + [: Previous session
      case "[":
        if (!isInput) {
          e.preventDefault();
          const sessions = appStore.sessions();
          const currentId = appStore.selectedSessionId();
          if (sessions.length > 0) {
            const currentIndex = sessions.findIndex((s) => s.id === currentId);
            const prevIndex =
              currentIndex <= 0 ? sessions.length - 1 : currentIndex - 1;
            appStore.setSelectedSessionId(sessions[prevIndex].id);
          }
        }
        break;
    }
  };

  onMount(() => {
    window.addEventListener("keydown", handleKeyDown);
  });

  onCleanup(() => {
    window.removeEventListener("keydown", handleKeyDown);
  });
}

// Export platform info for UI hints
export const shortcutModifier = isMac ? "⌘" : "Ctrl";
