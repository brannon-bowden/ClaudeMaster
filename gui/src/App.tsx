import { Show, onMount, createSignal, For } from "solid-js";
import { Sidebar } from "./components/Sidebar";
import { Terminal } from "./components/Terminal";
import { StatusBar } from "./components/StatusBar";
import { NewSessionDialog } from "./components/NewSessionDialog";
import { NewGroupDialog } from "./components/NewGroupDialog";
import { SettingsModal } from "./components/SettingsModal";
import { ToastContainer } from "./components/Toast";
import { appStore } from "./stores/appStore";
import { terminalStore } from "./stores/terminalStore";
import {
  useKeyboardShortcuts,
  shortcutModifier,
} from "./hooks/useKeyboardShortcuts";

function App() {
  const [isNewSessionOpen, setIsNewSessionOpen] = createSignal(false);
  const [isNewGroupOpen, setIsNewGroupOpen] = createSignal(false);
  const [isSettingsOpen, setIsSettingsOpen] = createSignal(false);

  // Set up keyboard shortcuts
  useKeyboardShortcuts({
    onNewSession: () => {
      if (appStore.isConnected()) {
        setIsNewSessionOpen(true);
      }
    },
    onNewGroup: () => {
      if (appStore.isConnected()) {
        setIsNewGroupOpen(true);
      }
    },
    onSettings: () => setIsSettingsOpen(true),
    onFocusSearch: () => {
      // Focus the search input in sidebar
      const searchInput = document.querySelector(
        'input[placeholder="Search sessions..."]'
      ) as HTMLInputElement;
      searchInput?.focus();
    },
  });

  // Auto-connect on mount
  onMount(() => {
    appStore.connectToDaemon();
  });

  return (
    <div class="h-screen bg-gray-900 text-white flex flex-col">
      <div class="flex-1 flex overflow-hidden">
      {/* Sidebar */}
      <Sidebar />

      {/* Main content area */}
      <div class="flex-1 flex flex-col">
        {/* Session header - only show when a session is selected */}
        <Show when={appStore.selectedSession}>
          {(session) => (
            <div class="px-3 py-1.5 border-b border-gray-700 flex items-center justify-between gap-2">
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <h2 class="text-sm font-semibold truncate">{session().name}</h2>
                  <span class="text-xs text-gray-500 truncate hidden sm:block">{session().working_dir}</span>
                </div>
              </div>
              <div class="flex items-center gap-1.5 flex-shrink-0">
                <Show when={session().status === "running"}>
                  <button
                    class="px-2 py-0.5 text-xs bg-yellow-600 hover:bg-yellow-700 rounded"
                    onClick={() => appStore.stopSession(session().id)}
                    title={`Stop session (${shortcutModifier}+W)`}
                  >
                    Stop
                  </button>
                </Show>
                <Show when={session().status === "stopped"}>
                  <button
                    class="px-2 py-0.5 text-xs bg-green-600 hover:bg-green-700 rounded"
                    onClick={() => {
                      // Get terminal dimensions if available, otherwise use defaults
                      const dims = terminalStore.getTerminalDimensions(session().id);
                      const rows = dims?.rows ?? 24;
                      const cols = dims?.cols ?? 80;
                      console.log(`[App] Restart button clicked, using size ${cols}x${rows}`);
                      appStore.restartSession(session().id, rows, cols);
                    }}
                    title={`Restart session (${shortcutModifier}+R)`}
                  >
                    Restart
                  </button>
                </Show>
                <Show when={session().claude_session_id}>
                  <button
                    class="px-2 py-0.5 text-xs bg-indigo-600 hover:bg-indigo-700 rounded"
                    onClick={() => {
                      const name = prompt(
                        "Fork name:",
                        `${session().name} (Fork)`
                      );
                      if (name) {
                        appStore.forkSession(session().id, name);
                      }
                    }}
                    title="Fork session"
                  >
                    Fork
                  </button>
                </Show>
                <button
                  class="px-2 py-0.5 text-xs bg-red-600 hover:bg-red-700 rounded"
                  onClick={() => {
                    if (
                      confirm(
                        `Delete session "${session().name}"? This cannot be undone.`
                      )
                    ) {
                      appStore.deleteSession(session().id);
                    }
                  }}
                  title="Delete session"
                >
                  Delete
                </button>
              </div>
            </div>
          )}
        </Show>

        {/* Terminal container - render ALL session terminals, show/hide based on selection */}
        {/* This preserves each terminal's buffer when switching between sessions */}
        <div class="flex-1 overflow-hidden relative">
          <Show
            when={appStore.sessions().length > 0}
            fallback={
              <div class="absolute inset-0 flex items-center justify-center text-gray-500">
                Select a session or create a new one
              </div>
            }
          >
            <For each={appStore.sessions()}>
              {(session) => (
                <div
                  class="absolute inset-0"
                  style={{
                    visibility: appStore.selectedSessionId() === session.id ? "visible" : "hidden",
                    "z-index": appStore.selectedSessionId() === session.id ? 1 : 0,
                  }}
                >
                  <Terminal
                    sessionId={session.id}
                    sessionStatus={session.status}
                  />
                </div>
              )}
            </For>
          </Show>
          {/* Show placeholder when no session is selected but sessions exist */}
          <Show when={appStore.sessions().length > 0 && !appStore.selectedSessionId()}>
            <div class="absolute inset-0 flex items-center justify-center text-gray-500">
              Select a session from the sidebar
            </div>
          </Show>
        </div>
      </div>
      </div>

      {/* Status bar */}
      <StatusBar />

      {/* Global dialogs triggered by keyboard shortcuts */}
      <NewSessionDialog
        isOpen={isNewSessionOpen()}
        onClose={() => setIsNewSessionOpen(false)}
      />
      <NewGroupDialog
        isOpen={isNewGroupOpen()}
        onClose={() => setIsNewGroupOpen(false)}
      />
      <SettingsModal
        isOpen={isSettingsOpen()}
        onClose={() => setIsSettingsOpen(false)}
      />

      {/* Toast notifications */}
      <ToastContainer />
    </div>
  );
}

export default App;
