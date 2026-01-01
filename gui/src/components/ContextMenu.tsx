// Reusable context menu component
// Renders at mouse position, closes on click outside or Escape

import { For, Show, createEffect, onCleanup, onMount } from "solid-js";
import { Portal } from "solid-js/web";

export interface ContextMenuItem {
  label: string;
  icon?: string; // SVG path data
  onClick: () => void;
  disabled?: boolean;
  danger?: boolean;
  separator?: boolean;
}

interface ContextMenuProps {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

export function ContextMenu(props: ContextMenuProps) {
  let menuRef: HTMLDivElement | undefined;

  // Adjust position to keep menu in viewport
  const getPosition = () => {
    const padding = 8;
    let x = props.x;
    let y = props.y;

    if (menuRef) {
      const rect = menuRef.getBoundingClientRect();
      if (x + rect.width > window.innerWidth - padding) {
        x = window.innerWidth - rect.width - padding;
      }
      if (y + rect.height > window.innerHeight - padding) {
        y = window.innerHeight - rect.height - padding;
      }
    }

    return { x: Math.max(padding, x), y: Math.max(padding, y) };
  };

  // Close on click outside
  const handleClickOutside = (e: MouseEvent) => {
    if (menuRef && !menuRef.contains(e.target as Node)) {
      props.onClose();
    }
  };

  // Close on escape
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      props.onClose();
    }
  };

  onMount(() => {
    // Small delay to avoid immediate close from the triggering click
    setTimeout(() => {
      document.addEventListener("mousedown", handleClickOutside);
    }, 0);
    document.addEventListener("keydown", handleKeyDown);
  });

  onCleanup(() => {
    document.removeEventListener("mousedown", handleClickOutside);
    document.removeEventListener("keydown", handleKeyDown);
  });

  // Reposition after mount
  createEffect(() => {
    if (menuRef) {
      const pos = getPosition();
      menuRef.style.left = `${pos.x}px`;
      menuRef.style.top = `${pos.y}px`;
    }
  });

  return (
    <Portal>
      <div
        ref={menuRef}
        class="fixed z-50 min-w-[160px] bg-gray-800 border border-gray-600 rounded-lg shadow-xl py-1 text-sm"
        style={{
          left: `${props.x}px`,
          top: `${props.y}px`,
        }}
      >
        <For each={props.items}>
          {(item) => (
            <Show
              when={!item.separator}
              fallback={<div class="h-px bg-gray-600 my-1" />}
            >
              <button
                class={`w-full px-3 py-1.5 text-left flex items-center gap-2 transition-colors ${
                  item.disabled
                    ? "text-gray-500 cursor-not-allowed"
                    : item.danger
                    ? "text-red-400 hover:bg-red-900/30"
                    : "text-gray-200 hover:bg-gray-700"
                }`}
                disabled={item.disabled}
                onClick={() => {
                  if (!item.disabled) {
                    item.onClick();
                    props.onClose();
                  }
                }}
              >
                <Show when={item.icon}>
                  <svg
                    class="w-4 h-4 flex-shrink-0"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d={item.icon}
                    />
                  </svg>
                </Show>
                <span>{item.label}</span>
              </button>
            </Show>
          )}
        </For>
      </div>
    </Portal>
  );
}

// Common icons as SVG path data
export const MenuIcons = {
  restart: "M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15",
  stop: "M21 12a9 9 0 11-18 0 9 9 0 0118 0z M9 10a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1h-4a1 1 0 01-1-1v-4z",
  play: "M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z M21 12a9 9 0 11-18 0 9 9 0 0118 0z",
  fork: "M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1",
  edit: "M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z",
  delete: "M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16",
  folder: "M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z",
  folderOpen: "M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z",
};
