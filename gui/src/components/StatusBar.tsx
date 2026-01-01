// Status bar component showing session counts and connection status

import { createMemo } from "solid-js";
import { appStore } from "../stores/appStore";
import { shortcutModifier } from "../hooks/useKeyboardShortcuts";

export function StatusBar() {
  const sessionCounts = createMemo(() => {
    const sessions = appStore.sessions();
    return {
      total: sessions.length,
      running: sessions.filter((s) => s.status === "running").length,
      waiting: sessions.filter((s) => s.status === "waiting").length,
      idle: sessions.filter((s) => s.status === "idle").length,
      stopped: sessions.filter((s) => s.status === "stopped").length,
      error: sessions.filter((s) => s.status === "error").length,
    };
  });

  return (
    <div class="h-6 bg-gray-800 border-t border-gray-700 px-2 flex items-center justify-between text-xs text-gray-400">
      {/* Left side: Session counts */}
      <div class="flex items-center gap-4">
        <span class="flex items-center gap-1">
          <span class="w-2 h-2 rounded-full bg-gray-500" />
          <span>{sessionCounts().total} sessions</span>
        </span>
        <span class="flex items-center gap-1">
          <span class="w-2 h-2 rounded-full bg-green-500" />
          <span>{sessionCounts().running} running</span>
        </span>
        <span class="flex items-center gap-1">
          <span class="w-2 h-2 rounded-full bg-yellow-500" />
          <span>{sessionCounts().waiting} waiting</span>
        </span>
        {sessionCounts().idle > 0 && (
          <span class="flex items-center gap-1">
            <span class="w-2 h-2 rounded-full bg-blue-500" />
            <span>{sessionCounts().idle} idle</span>
          </span>
        )}
        {sessionCounts().error > 0 && (
          <span class="flex items-center gap-1">
            <span class="w-2 h-2 rounded-full bg-red-500" />
            <span>{sessionCounts().error} error</span>
          </span>
        )}
      </div>

      {/* Right side: Shortcuts hint and connection status */}
      <div class="flex items-center gap-4">
        <span class="text-gray-500">
          {shortcutModifier}+N New session • {shortcutModifier}+K Search •{" "}
          {shortcutModifier}+, Settings
        </span>
        <span
          class={`flex items-center gap-1 ${
            appStore.isConnected() ? "text-green-400" : "text-red-400"
          }`}
        >
          <span
            class={`w-2 h-2 rounded-full ${
              appStore.isConnected() ? "bg-green-400" : "bg-red-400"
            }`}
          />
          {appStore.isConnected() ? "Connected" : "Disconnected"}
        </span>
      </div>
    </div>
  );
}
