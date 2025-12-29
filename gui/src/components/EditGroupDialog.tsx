// Dialog for editing a group (name and parent)

import { createSignal, Show, For, createEffect } from "solid-js";
import { appStore } from "../stores/appStore";
import type { Group } from "../types";

interface EditGroupDialogProps {
  isOpen: boolean;
  onClose: () => void;
  group: Group | null;
}

export function EditGroupDialog(props: EditGroupDialogProps) {
  const [name, setName] = createSignal("");
  const [parentId, setParentId] = createSignal<string | null>(null);
  const [isUpdating, setIsUpdating] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  // Update form when group changes
  createEffect(() => {
    if (props.group) {
      setName(props.group.name);
      setParentId(props.group.parent_id || null);
    }
  });

  // Filter out the current group and its descendants to prevent circular references
  const availableParents = () => {
    if (!props.group) return appStore.groups();

    // Build a set of descendant IDs
    const descendants = new Set<string>();
    const findDescendants = (groupId: string) => {
      for (const g of appStore.groups()) {
        if (g.parent_id === groupId) {
          descendants.add(g.id);
          findDescendants(g.id);
        }
      }
    };
    descendants.add(props.group.id);
    findDescendants(props.group.id);

    return appStore.groups().filter((g) => !descendants.has(g.id));
  };

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    if (!props.group) return;
    if (!name().trim()) {
      setError("Group name is required");
      return;
    }

    setIsUpdating(true);
    setError(null);

    try {
      await appStore.updateGroup(
        props.group.id,
        name().trim(),
        parentId()
      );
      props.onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setIsUpdating(false);
    }
  };

  const handleClose = () => {
    setError(null);
    props.onClose();
  };

  return (
    <Show when={props.isOpen && props.group}>
      <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
        <div class="bg-gray-800 rounded-lg shadow-xl w-full max-w-md mx-4">
          <form onSubmit={handleSubmit}>
            <div class="p-4 border-b border-gray-700">
              <h2 class="text-lg font-semibold text-white">Edit Group</h2>
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

              {/* Parent Group Selection */}
              <div>
                <label class="block text-sm font-medium text-gray-300 mb-1">
                  Parent Group
                </label>
                <select
                  value={parentId() || ""}
                  onChange={(e) => setParentId(e.currentTarget.value || null)}
                  class="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded-md text-white focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent"
                >
                  <option value="">No parent (root level)</option>
                  <For each={availableParents()}>
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
                disabled={isUpdating()}
                class="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-700 disabled:opacity-50 disabled:cursor-not-allowed rounded-md text-white transition-colors"
              >
                {isUpdating() ? "Saving..." : "Save Changes"}
              </button>
            </div>
          </form>
        </div>
      </div>
    </Show>
  );
}
