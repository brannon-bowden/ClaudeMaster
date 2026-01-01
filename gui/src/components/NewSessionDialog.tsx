// New Session Dialog Component

import { createSignal, Show, For } from "solid-js";
import { open } from "@tauri-apps/plugin-dialog";
import { appStore } from "../stores/appStore";

interface NewSessionDialogProps {
  isOpen: boolean;
  onClose: () => void;
  groupId?: string;
}

export function NewSessionDialog(props: NewSessionDialogProps) {
  const [name, setName] = createSignal("");
  const [directory, setDirectory] = createSignal("");
  const [selectedGroupId, setSelectedGroupId] = createSignal<string | undefined>(props.groupId);
  const [isCreating, setIsCreating] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const handleBrowse = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select Working Directory",
      });
      if (selected) {
        setDirectory(selected as string);
      }
    } catch (e) {
      console.error("Failed to open directory picker:", e);
    }
  };

  const handleCreate = async () => {
    if (!name().trim()) {
      setError("Please enter a session name");
      return;
    }
    if (!directory().trim()) {
      setError("Please select a working directory");
      return;
    }

    setIsCreating(true);
    setError(null);

    try {
      await appStore.createSession(name().trim(), directory().trim(), selectedGroupId());
      // Reset and close
      setName("");
      setDirectory("");
      setSelectedGroupId(undefined);
      props.onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsCreating(false);
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      props.onClose();
    } else if (e.key === "Enter" && !isCreating()) {
      handleCreate();
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
          class="bg-gray-800 rounded-lg shadow-xl w-full max-w-md p-6"
          onClick={(e) => e.stopPropagation()}
        >
          <h2 class="text-xl font-semibold text-white mb-4">New Session</h2>

          <div class="space-y-4">
            {/* Session Name */}
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-1">
                Session Name
              </label>
              <input
                type="text"
                value={name()}
                onInput={(e) => setName(e.currentTarget.value)}
                placeholder="My Claude Session"
                class="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded-md text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent"
                autofocus
              />
            </div>

            {/* Working Directory */}
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-1">
                Working Directory
              </label>
              <div class="flex gap-2">
                <input
                  type="text"
                  value={directory()}
                  onInput={(e) => setDirectory(e.currentTarget.value)}
                  placeholder="/path/to/project"
                  class="flex-1 px-3 py-2 bg-gray-700 border border-gray-600 rounded-md text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent"
                />
                <button
                  type="button"
                  onClick={handleBrowse}
                  class="px-3 py-2 bg-gray-600 hover:bg-gray-500 rounded-md text-white transition-colors"
                >
                  Browse
                </button>
              </div>
            </div>

            {/* Group Selection */}
            <Show when={appStore.groups().length > 0}>
              <div>
                <label class="block text-sm font-medium text-gray-300 mb-1">
                  Group (optional)
                </label>
                <select
                  value={selectedGroupId() || ""}
                  onChange={(e) => setSelectedGroupId(e.currentTarget.value || undefined)}
                  class="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded-md text-white focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent"
                >
                  <option value="">No group</option>
                  <For each={appStore.groups()}>
                    {(group) => (
                      <option value={group.id}>{group.name}</option>
                    )}
                  </For>
                </select>
              </div>
            </Show>

            {/* Error Message */}
            <Show when={error()}>
              <p class="text-sm text-red-400">{error()}</p>
            </Show>
          </div>

          {/* Actions */}
          <div class="flex justify-end gap-3 mt-6">
            <button
              type="button"
              onClick={props.onClose}
              class="px-4 py-2 text-sm text-gray-300 hover:text-white transition-colors"
              disabled={isCreating()}
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleCreate}
              disabled={isCreating()}
              class="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-700 rounded-md text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isCreating() ? "Creating..." : "Create Session"}
            </button>
          </div>
        </div>
      </div>
    </Show>
  );
}
