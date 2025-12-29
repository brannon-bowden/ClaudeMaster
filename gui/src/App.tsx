import { Show, onMount, onCleanup, createSignal } from "solid-js";
import { Sidebar } from "./components/Sidebar";
import { Terminal } from "./components/Terminal";
import { NewSessionDialog } from "./components/NewSessionDialog";
import { SettingsModal } from "./components/SettingsModal";
import { ToastContainer } from "./components/Toast";
import { appStore } from "./stores/appStore";

function App() {
  const [isNewSessionOpen, setIsNewSessionOpen] = createSignal(false);
  const [isSettingsOpen, setIsSettingsOpen] = createSignal(false);

  // Keyboard shortcuts
  const handleKeyDown = (e: KeyboardEvent) => {
    const isMod = e.metaKey || e.ctrlKey;

    if (isMod && e.key === "n") {
      e.preventDefault();
      if (appStore.isConnected()) {
        setIsNewSessionOpen(true);
      }
    } else if (isMod && e.key === ",") {
      e.preventDefault();
      setIsSettingsOpen(true);
    } else if (isMod && e.key === "w") {
      e.preventDefault();
      const session = appStore.selectedSession;
      if (session && session.status === "Running") {
        appStore.stopSession(session.id);
      }
    }
  };

  // Auto-connect on mount
  onMount(() => {
    appStore.connectToDaemon();
    document.addEventListener("keydown", handleKeyDown);
  });

  onCleanup(() => {
    document.removeEventListener("keydown", handleKeyDown);
  });

  return (
    <div class="h-screen bg-gray-900 text-white flex">
      {/* Sidebar */}
      <Sidebar />

      {/* Main content area */}
      <div class="flex-1 flex flex-col">
        <Show
          when={appStore.selectedSession}
          fallback={
            <div class="flex-1 flex items-center justify-center text-gray-500">
              Select a session or create a new one
            </div>
          }
        >
          {(session) => (
            <div class="flex-1 flex flex-col">
              {/* Session header */}
              <div class="p-4 border-b border-gray-700 flex items-center justify-between">
                <div>
                  <h2 class="text-lg font-semibold">{session().name}</h2>
                  <p class="text-sm text-gray-400">{session().working_dir}</p>
                </div>
                <div class="flex items-center gap-2">
                  <Show when={session().status === "Running"}>
                    <button
                      class="px-3 py-1.5 text-sm bg-yellow-600 hover:bg-yellow-700 rounded-md"
                      onClick={() => appStore.stopSession(session().id)}
                    >
                      Stop
                    </button>
                  </Show>
                  <Show when={session().claude_session_id}>
                    <button
                      class="px-3 py-1.5 text-sm bg-indigo-600 hover:bg-indigo-700 rounded-md"
                      onClick={() => {
                        const name = prompt(
                          "Fork name:",
                          `${session().name} (Fork)`
                        );
                        if (name) {
                          appStore.forkSession(session().id, name);
                        }
                      }}
                    >
                      Fork
                    </button>
                  </Show>
                  <button
                    class="px-3 py-1.5 text-sm bg-red-600 hover:bg-red-700 rounded-md"
                    onClick={() => {
                      if (
                        confirm(
                          `Delete session "${session().name}"? This cannot be undone.`
                        )
                      ) {
                        appStore.deleteSession(session().id);
                      }
                    }}
                  >
                    Delete
                  </button>
                </div>
              </div>

              {/* Terminal */}
              <div class="flex-1 overflow-hidden">
                <Terminal sessionId={session().id} />
              </div>
            </div>
          )}
        </Show>
      </div>

      {/* Global dialogs triggered by keyboard shortcuts */}
      <NewSessionDialog
        isOpen={isNewSessionOpen()}
        onClose={() => setIsNewSessionOpen(false)}
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
