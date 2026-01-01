// Settings Modal Component

import { createSignal, Show } from "solid-js";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

// Local settings state (persisted via localStorage)
const [theme, setTheme] = createSignal(
  localStorage.getItem("theme") || "dark"
);
const [fontSize, setFontSize] = createSignal(
  parseInt(localStorage.getItem("fontSize") || "14", 10)
);
const [fontFamily, setFontFamily] = createSignal(
  localStorage.getItem("fontFamily") || "JetBrains Mono"
);

export function SettingsModal(props: SettingsModalProps) {
  const handleSave = () => {
    localStorage.setItem("theme", theme());
    localStorage.setItem("fontSize", fontSize().toString());
    localStorage.setItem("fontFamily", fontFamily());
    props.onClose();
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      props.onClose();
    }
  };

  return (
    <Show when={props.isOpen}>
      <div
        class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
        onClick={() => props.onClose()}
        onKeyDown={handleKeyDown}
      >
        <div
          class="bg-gray-800 rounded-lg shadow-xl w-full max-w-lg p-6"
          onClick={(e) => e.stopPropagation()}
        >
          <h2 class="text-xl font-semibold text-white mb-6">Settings</h2>

          <div class="space-y-6">
            {/* Theme */}
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-2">
                Theme
              </label>
              <div class="flex gap-4">
                <label class="flex items-center gap-2 cursor-pointer">
                  <input
                    type="radio"
                    name="theme"
                    value="dark"
                    checked={theme() === "dark"}
                    onChange={() => setTheme("dark")}
                    class="text-indigo-600 focus:ring-indigo-500"
                  />
                  <span class="text-gray-300">Dark</span>
                </label>
                <label class="flex items-center gap-2 cursor-pointer">
                  <input
                    type="radio"
                    name="theme"
                    value="light"
                    checked={theme() === "light"}
                    onChange={() => setTheme("light")}
                    class="text-indigo-600 focus:ring-indigo-500"
                  />
                  <span class="text-gray-300">Light</span>
                </label>
              </div>
            </div>

            {/* Font Size */}
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-2">
                Terminal Font Size: {fontSize()}px
              </label>
              <input
                type="range"
                min="10"
                max="24"
                value={fontSize()}
                onInput={(e) => setFontSize(parseInt(e.currentTarget.value, 10))}
                class="w-full h-2 bg-gray-700 rounded-lg appearance-none cursor-pointer"
              />
            </div>

            {/* Font Family */}
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-2">
                Terminal Font
              </label>
              <select
                value={fontFamily()}
                onChange={(e) => setFontFamily(e.currentTarget.value)}
                class="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded-md text-white focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent"
              >
                <option value="JetBrains Mono">JetBrains Mono</option>
                <option value="Fira Code">Fira Code</option>
                <option value="SF Mono">SF Mono</option>
                <option value="Monaco">Monaco</option>
                <option value="Menlo">Menlo</option>
                <option value="Consolas">Consolas</option>
                <option value="monospace">System Monospace</option>
              </select>
            </div>

            {/* Keyboard shortcuts info */}
            <div class="pt-4 border-t border-gray-700">
              <h3 class="text-sm font-medium text-gray-300 mb-3">
                Keyboard Shortcuts
              </h3>
              <div class="space-y-2 text-sm text-gray-400">
                <div class="flex justify-between">
                  <span>New Session</span>
                  <kbd class="px-2 py-0.5 bg-gray-700 rounded text-gray-300">
                    Cmd/Ctrl + N
                  </kbd>
                </div>
                <div class="flex justify-between">
                  <span>Close Session</span>
                  <kbd class="px-2 py-0.5 bg-gray-700 rounded text-gray-300">
                    Cmd/Ctrl + W
                  </kbd>
                </div>
                <div class="flex justify-between">
                  <span>Settings</span>
                  <kbd class="px-2 py-0.5 bg-gray-700 rounded text-gray-300">
                    Cmd/Ctrl + ,
                  </kbd>
                </div>
                <div class="flex justify-between">
                  <span>Focus Terminal</span>
                  <kbd class="px-2 py-0.5 bg-gray-700 rounded text-gray-300">
                    Cmd/Ctrl + `
                  </kbd>
                </div>
              </div>
            </div>
          </div>

          {/* Actions */}
          <div class="flex justify-end gap-3 mt-6">
            <button
              type="button"
              onClick={props.onClose}
              class="px-4 py-2 text-sm text-gray-300 hover:text-white transition-colors"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleSave}
              class="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-700 rounded-md text-white transition-colors"
            >
              Save
            </button>
          </div>
        </div>
      </div>
    </Show>
  );
}

// Export settings for use elsewhere
export { theme, fontSize, fontFamily };
