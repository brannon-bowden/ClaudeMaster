// Dialog for creating a new group

import { createSignal, Show, For } from "solid-js";
import { appStore } from "../stores/appStore";

interface NewGroupDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export function NewGroupDialog(props: NewGroupDialogProps) {
  const [name, setName] = createSignal("");
  const [parentId, setParentId] = createSignal<string | undefined>(undefined);
  const [isCreating, setIsCreating] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    if (!name().trim()) {
      setError("Group name is required");
      return;
    }

    setIsCreating(true);
    setError(null);

    try {
      await appStore.createGroup(name().trim(), parentId());
      // Reset and close
      setName("");
      setParentId(undefined);
      props.onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setIsCreating(false);
    }
  };

  const handleClose = () => {
    setName("");
    setParentId(undefined);
    setError(null);
    props.onClose();
  };

  return (
    <Show when={props.isOpen}>
      <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
        <div class="bg-gray-800 rounded-lg shadow-xl w-full max-w-md mx-4">
          <form onSubmit={handleSubmit}>
            <div class="p-4 border-b border-gray-700">
              <h2 class="text-lg font-semibold text-white">New Group</h2>
            </div>

            <div class="p-4 space-y-4">
              {/* Group Name */}
              <div>
                <label class="block text-sm font-medium text-gray-300 mb-1">
                  Group Name
                </label>
                <input
                  type="text"
                  value={name()}
                  onInput={(e) => setName(e.currentTarget.value)}
                  class="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded-md text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent"
                  placeholder="Enter group name"
                  autofocus
                />
              </div>

              {/* Parent Group (optional) */}
              <div>
                <label class="block text-sm font-medium text-gray-300 mb-1">
                  Parent Group (optional)
                </label>
                <select
                  value={parentId() || ""}
                  onChange={(e) => setParentId(e.currentTarget.value || undefined)}
                  class="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded-md text-white focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent"
                >
                  <option value="">No parent (root level)</option>
                  <For each={appStore.groups()}>
                    {(group) => (
                      <option value={group.id}>{group.name}</option>
                    )}
                  </For>
                </select>
              </div>

              {/* Error message */}
              <Show when={error()}>
                <p class="text-sm text-red-400">{error()}</p>
              </Show>
            </div>

            <div class="p-4 border-t border-gray-700 flex justify-end gap-2">
              <button
                type="button"
                onClick={handleClose}
                class="px-4 py-2 text-sm text-gray-300 hover:text-white transition-colors"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={isCreating()}
                class="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-700 disabled:opacity-50 disabled:cursor-not-allowed rounded-md text-white transition-colors"
              >
                {isCreating() ? "Creating..." : "Create Group"}
              </button>
            </div>
          </form>
        </div>
      </div>
    </Show>
  );
}
