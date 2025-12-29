// Sidebar component with nested groups and sessions

import { For, Show, createMemo, createSignal } from "solid-js";
import { appStore } from "../stores/appStore";
import { NewSessionDialog } from "./NewSessionDialog";
import { SettingsModal } from "./SettingsModal";
import type { GroupNode, Session, SessionStatus } from "../types";

// Status indicator colors
const statusColors: Record<SessionStatus, string> = {
  Stopped: "bg-gray-500",
  Running: "bg-green-500",
  Waiting: "bg-yellow-500",
  Error: "bg-red-500",
  Completed: "bg-blue-500",
};

// Session item component
function SessionItem(props: { session: Session }) {
  const isSelected = createMemo(
    () => appStore.selectedSessionId() === props.session.id
  );

  return (
    <div
      class={`flex items-center gap-2 px-3 py-2 cursor-pointer rounded-md transition-colors ${
        isSelected()
          ? "bg-indigo-600 text-white"
          : "hover:bg-gray-700 text-gray-300"
      }`}
      onClick={() => appStore.setSelectedSessionId(props.session.id)}
    >
      <span
        class={`w-2 h-2 rounded-full ${statusColors[props.session.status]}`}
        title={props.session.status}
      />
      <span class="truncate flex-1">{props.session.name}</span>
    </div>
  );
}

// Group component (recursive for nesting)
function GroupItem(props: { group: GroupNode; depth?: number }) {
  const depth = props.depth || 0;
  const paddingLeft = `${depth * 12 + 8}px`;

  return (
    <div class="select-none">
      {/* Group header */}
      <div
        class="flex items-center gap-2 px-2 py-1.5 cursor-pointer hover:bg-gray-700 rounded-md text-gray-400"
        style={{ "padding-left": paddingLeft }}
        onClick={() => appStore.toggleGroupCollapse(props.group.id)}
      >
        <svg
          class={`w-4 h-4 transition-transform ${
            props.group.collapsed ? "" : "rotate-90"
          }`}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M9 5l7 7-7 7"
          />
        </svg>
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
          />
        </svg>
        <span class="truncate flex-1 text-sm font-medium">
          {props.group.name}
        </span>
        <span class="text-xs text-gray-500">
          {props.group.sessions.length + props.group.children.length}
        </span>
      </div>

      {/* Group contents (children and sessions) */}
      <Show when={!props.group.collapsed}>
        <div class="ml-2">
          <For each={props.group.children}>
            {(child) => <GroupItem group={child} depth={depth + 1} />}
          </For>
          <For each={props.group.sessions}>
            {(session) => (
              <div style={{ "padding-left": `${(depth + 1) * 12}px` }}>
                <SessionItem session={session} />
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}

// Main Sidebar component
export function Sidebar() {
  const tree = createMemo(() => appStore.groupTree);
  const [isNewSessionOpen, setIsNewSessionOpen] = createSignal(false);
  const [isSettingsOpen, setIsSettingsOpen] = createSignal(false);

  return (
    <div class="w-64 bg-gray-800 border-r border-gray-700 flex flex-col h-full">
      {/* Header */}
      <div class="p-4 border-b border-gray-700">
        <h1 class="text-lg font-semibold text-white">Claude Master</h1>
        <Show when={appStore.isConnected()}>
          <p class="text-xs text-green-400 mt-1">Connected to daemon</p>
        </Show>
        <Show when={!appStore.isConnected()}>
          <button
            class="mt-2 w-full px-3 py-1.5 text-sm bg-indigo-600 hover:bg-indigo-700 rounded-md text-white transition-colors"
            onClick={() => appStore.connectToDaemon()}
          >
            Connect
          </button>
          <Show when={appStore.connectionError()}>
            <p class="text-xs text-red-400 mt-1">{appStore.connectionError()}</p>
          </Show>
        </Show>
      </div>

      {/* Session/Group list */}
      <div class="flex-1 overflow-y-auto p-2">
        {/* Render group tree */}
        <For each={tree().roots}>{(group) => <GroupItem group={group} />}</For>

        {/* Render orphan sessions (not in any group) */}
        <Show when={tree().orphanSessions.length > 0}>
          <div class="mt-2 pt-2 border-t border-gray-700">
            <For each={tree().orphanSessions}>
              {(session) => <SessionItem session={session} />}
            </For>
          </div>
        </Show>

        {/* Empty state */}
        <Show
          when={
            tree().roots.length === 0 && tree().orphanSessions.length === 0
          }
        >
          <div class="text-gray-500 text-sm text-center py-8">
            No sessions yet
          </div>
        </Show>
      </div>

      {/* Footer actions */}
      <div class="p-2 border-t border-gray-700 space-y-2">
        <button
          class="w-full flex items-center justify-center gap-2 px-3 py-2 text-sm bg-gray-700 hover:bg-gray-600 rounded-md text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          disabled={!appStore.isConnected()}
          onClick={() => setIsNewSessionOpen(true)}
        >
          <svg
            class="w-4 h-4"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M12 4v16m8-8H4"
            />
          </svg>
          New Session
        </button>
        <button
          class="w-full flex items-center justify-center gap-2 px-3 py-2 text-sm text-gray-400 hover:text-white hover:bg-gray-700 rounded-md transition-colors"
          onClick={() => setIsSettingsOpen(true)}
        >
          <svg
            class="w-4 h-4"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
            />
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
            />
          </svg>
          Settings
        </button>
      </div>

      {/* New Session Dialog */}
      <NewSessionDialog
        isOpen={isNewSessionOpen()}
        onClose={() => setIsNewSessionOpen(false)}
      />

      {/* Settings Modal */}
      <SettingsModal
        isOpen={isSettingsOpen()}
        onClose={() => setIsSettingsOpen(false)}
      />
    </div>
  );
}
