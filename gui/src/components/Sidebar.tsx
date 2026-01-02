// Sidebar component with nested groups and sessions
// Supports keyboard navigation: ↑/↓ to navigate, Enter to select, ←/→ to collapse/expand
// Supports drag and drop for reordering sessions and groups

import { For, Show, createMemo, createSignal, createEffect, onMount, onCleanup } from "solid-js";
import {
  DragDropProvider,
  DragDropSensors,
  DragOverlay,
  SortableProvider,
  createSortable,
  closestCenter,
  type Id,
  type DragEvent,
} from "@thisbeyond/solid-dnd";
import { appStore, buildGroupTree } from "../stores/appStore";
import { NewSessionDialog } from "./NewSessionDialog";
import { NewGroupDialog } from "./NewGroupDialog";
import { EditSessionDialog } from "./EditSessionDialog";
import { EditGroupDialog } from "./EditGroupDialog";
import { SettingsModal } from "./SettingsModal";
import { ContextMenu, ContextMenuItem, MenuIcons } from "./ContextMenu";
import { StatusPill } from "./StatusPill";
import type { Group, GroupNode, Session, SessionStatus } from "../types";

// Type for navigable items in the sidebar
type NavItem =
  | { type: "session"; session: Session; depth: number; groupId: string | null }
  | { type: "group"; group: GroupNode; depth: number; parentId: string | null };

// Generate a unique drag ID for items (encodes type and actual ID)
function makeDragId(type: "session" | "group", id: string): Id {
  return `${type}:${id}`;
}

// Parse drag ID back to type and id
function parseDragId(dragId: Id): { type: "session" | "group"; id: string } | null {
  const str = String(dragId);
  const [type, id] = str.split(":");
  if ((type === "session" || type === "group") && id) {
    return { type, id };
  }
  return null;
}

// Flatten tree into navigable items (respecting collapsed state)
function flattenTree(tree: { roots: GroupNode[]; orphanSessions: Session[] }): NavItem[] {
  const items: NavItem[] = [];

  function addGroup(group: GroupNode, depth: number, parentId: string | null) {
    items.push({ type: "group", group, depth, parentId });
    if (!group.collapsed) {
      for (const child of group.children) {
        addGroup(child, depth + 1, group.id);
      }
      for (const session of group.sessions) {
        items.push({ type: "session", session, depth: depth + 1, groupId: group.id });
      }
    }
  }

  for (const group of tree.roots) {
    addGroup(group, 0, null);
  }

  for (const session of tree.orphanSessions) {
    items.push({ type: "session", session, depth: 0, groupId: null });
  }

  return items;
}

// Session item component
function SessionItem(props: {
  session: Session;
  depth: number;
  isFocused: boolean;
  onFocus: () => void;
  onContextMenu: (session: Session, x: number, y: number) => void;
  listContainerRef?: HTMLDivElement;
}) {
  let itemRef: HTMLDivElement | undefined;

  const isSelected = createMemo(
    () => appStore.selectedSessionId() === props.session.id
  );

  // Scroll into view when focused
  createEffect(() => {
    if (props.isFocused && itemRef) {
      itemRef.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
  });

  const handleContextMenu = (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    props.onContextMenu(props.session, e.clientX, e.clientY);
  };

  const paddingLeft = `${props.depth * 12 + 12}px`;

  return (
    <div
      ref={itemRef}
      class={`flex items-center gap-1.5 px-2 py-1 cursor-pointer rounded transition-colors text-sm ${
        isSelected()
          ? "bg-indigo-600 text-white"
          : "hover:bg-gray-700 text-gray-300"
      } ${props.isFocused ? "ring-1 ring-indigo-400 ring-inset" : ""}`}
      style={{ "padding-left": paddingLeft }}
      onClick={() => {
        props.onFocus();
        props.listContainerRef?.focus();
        appStore.setSelectedSessionId(props.session.id);
      }}
      onContextMenu={handleContextMenu}
      tabIndex={-1}
    >
      <span class="truncate flex-1">{props.session.name}</span>
      <StatusPill status={props.session.status} />
    </div>
  );
}

// Sortable wrapper for SessionItem (for drag and drop)
function SortableSession(props: {
  session: Session;
  depth: number;
  groupId: string | null;
  isFocused: boolean;
  onFocus: () => void;
  onContextMenu: (session: Session, x: number, y: number) => void;
  listContainerRef?: HTMLDivElement;
}) {
  const sortable = createSortable(makeDragId("session", props.session.id));
  const isSelected = createMemo(() => appStore.selectedSessionId() === props.session.id);
  const paddingLeft = `${props.depth * 12 + 12}px`;

  const handleContextMenu = (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    props.onContextMenu(props.session, e.clientX, e.clientY);
  };

  return (
    <div
      ref={sortable.ref}
      class={`flex items-center gap-1.5 px-2 py-1 cursor-grab rounded transition-colors text-sm ${
        isSelected()
          ? "bg-indigo-600 text-white"
          : "hover:bg-gray-700 text-gray-300"
      } ${props.isFocused ? "ring-1 ring-indigo-400 ring-inset" : ""} ${
        sortable.isActiveDraggable ? "opacity-50 cursor-grabbing" : ""
      }`}
      style={{ "padding-left": paddingLeft }}
      onClick={() => {
        if (!sortable.isActiveDraggable) {
          props.onFocus();
          props.listContainerRef?.focus();
          appStore.setSelectedSessionId(props.session.id);
        }
      }}
      onContextMenu={handleContextMenu}
      tabIndex={-1}
      {...sortable.dragActivators}
    >
      <span class="truncate flex-1">{props.session.name}</span>
      <StatusPill status={props.session.status} />
    </div>
  );
}

// Sortable wrapper for GroupHeader (for drag and drop)
function SortableGroup(props: {
  group: GroupNode;
  depth: number;
  parentId: string | null;
  isFocused: boolean;
  onEditGroup: (group: Group) => void;
  onFocus: () => void;
  onToggle: () => void;
  onContextMenu: (group: Group, x: number, y: number) => void;
  listContainerRef?: HTMLDivElement;
}) {
  const sortable = createSortable(makeDragId("group", props.group.id));
  const paddingLeft = `${props.depth * 12 + 8}px`;

  const handleEditGroup = (e: MouseEvent) => {
    e.stopPropagation();
    props.onEditGroup(props.group);
  };

  const handleContextMenu = (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    props.onContextMenu(props.group, e.clientX, e.clientY);
  };

  return (
    <div
      ref={sortable.ref}
      class={`group flex items-center gap-1 px-1.5 py-0.5 cursor-grab hover:bg-gray-700 rounded text-gray-400 select-none text-sm ${
        props.isFocused ? "ring-1 ring-indigo-400 ring-inset" : ""
      } ${sortable.isActiveDraggable ? "opacity-50 cursor-grabbing" : ""}`}
      style={{ "padding-left": paddingLeft }}
      onClick={() => {
        if (!sortable.isActiveDraggable) {
          props.onFocus();
          props.listContainerRef?.focus();
          props.onToggle();
        }
      }}
      onContextMenu={handleContextMenu}
      tabIndex={-1}
      {...sortable.dragActivators}
    >
      <svg
        class={`w-3 h-3 flex-shrink-0 transition-transform ${
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
      <svg
        class="w-3 h-3 flex-shrink-0 text-gray-500"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
        />
      </svg>
      <span class="truncate flex-1 font-medium">{props.group.name}</span>
      <span class="text-xs text-gray-500 flex-shrink-0">
        {props.group.sessions.length + props.group.children.length}
      </span>
      <button
        class="p-0.5 rounded opacity-0 group-hover:opacity-100 hover:bg-gray-600 transition-opacity"
        onClick={handleEditGroup}
        title="Edit group"
        tabIndex={-1}
      >
        <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"
          />
        </svg>
      </button>
    </div>
  );
}

// Context menu state type
type ContextMenuState =
  | { type: "session"; session: Session; x: number; y: number }
  | { type: "group"; group: Group; x: number; y: number }
  | null;

// Main Sidebar component
export function Sidebar() {
  // Access stores directly in memo to ensure SolidJS tracks dependencies properly
  // (getters on plain objects don't reliably trigger reactive updates)
  const tree = createMemo(() => buildGroupTree(appStore.groups(), appStore.sessions()));
  const [isNewSessionOpen, setIsNewSessionOpen] = createSignal(false);
  const [isNewGroupOpen, setIsNewGroupOpen] = createSignal(false);
  const [isSettingsOpen, setIsSettingsOpen] = createSignal(false);
  const [searchQuery, setSearchQuery] = createSignal("");
  const [focusedIndex, setFocusedIndex] = createSignal(-1);
  const [hasSidebarFocus, setHasSidebarFocus] = createSignal(false);
  const [contextMenu, setContextMenu] = createSignal<ContextMenuState>(null);

  let listContainerRef: HTMLDivElement | undefined;
  let searchInputRef: HTMLInputElement | undefined;

  // Edit dialog states
  const [editingSession, setEditingSession] = createSignal<Session | null>(null);
  const [editingGroup, setEditingGroup] = createSignal<Group | null>(null);

  // Filter sessions based on search query
  const filteredSessions = createMemo(() => {
    const query = searchQuery().toLowerCase().trim();
    if (!query) return null;
    return appStore.sessions().filter((session) =>
      session.name.toLowerCase().includes(query)
    );
  });

  // Get navigable items
  const navItems = createMemo((): NavItem[] => {
    const filtered = filteredSessions();
    if (filtered) {
      return filtered.map((session) => ({
        type: "session",
        session,
        depth: 0,
        groupId: session.group_id || null,
      }));
    }
    return flattenTree(tree());
  });

  // Active drag item for overlay
  const [activeDragItem, setActiveDragItem] = createSignal<NavItem | null>(null);

  // Reset focus when items change significantly
  createEffect(() => {
    const items = navItems();
    const currentFocus = focusedIndex();
    if (currentFocus >= items.length) {
      setFocusedIndex(items.length > 0 ? items.length - 1 : -1);
    }
  });

  const handleEditGroup = (group: Group) => {
    setEditingGroup(group);
  };

  // Drag and drop handler
  const handleDragEnd = async (event: DragEvent) => {
    setActiveDragItem(null);

    const { draggable, droppable } = event;
    if (!draggable || !droppable) return;

    const dragInfo = parseDragId(draggable.id);
    const dropInfo = parseDragId(droppable.id);

    if (!dragInfo || !dropInfo) return;
    if (draggable.id === droppable.id) return;

    const items = navItems();
    const dragIndex = items.findIndex((item) =>
      item.type === "session"
        ? item.session.id === dragInfo.id
        : item.type === "group" && item.group.id === dragInfo.id
    );
    const dropIndex = items.findIndex((item) =>
      item.type === "session"
        ? item.session.id === dropInfo.id
        : item.type === "group" && item.group.id === dropInfo.id
    );

    if (dragIndex === -1 || dropIndex === -1) return;

    const dropItem = items[dropIndex];

    try {
      if (dragInfo.type === "session") {
        // Session being reordered
        // Determine target group and position
        let targetGroupId: string | null = null;
        let afterSessionId: string | null = null;

        if (dropItem.type === "group") {
          // Dropped on a group - move into that group at beginning
          targetGroupId = dropItem.group.id;
          afterSessionId = null;
        } else {
          // Dropped on another session - use same group and place after it
          targetGroupId = dropItem.groupId;
          afterSessionId = dropItem.session.id;
        }

        await appStore.reorderSession(dragInfo.id, targetGroupId, afterSessionId);
      } else if (dragInfo.type === "group") {
        // Group being reordered
        // Determine target parent and position
        let targetParentId: string | null = null;
        let afterGroupId: string | null = null;

        if (dropItem.type === "group") {
          // Dropped on another group - become sibling, placed after it
          targetParentId = dropItem.parentId;
          afterGroupId = dropItem.group.id;
        } else {
          // Dropped on a session - move to same level as session's group (or root)
          targetParentId = null;
          afterGroupId = null;
        }

        await appStore.reorderGroup(dragInfo.id, targetParentId, afterGroupId);
      }
    } catch (e) {
      console.error("Failed to reorder:", e);
    }
  };

  const handleDragStart = (event: DragEvent) => {
    const { draggable } = event;
    if (!draggable) return;

    const dragInfo = parseDragId(draggable.id);
    if (!dragInfo) return;

    const items = navItems();
    const item = items.find((i) =>
      i.type === "session"
        ? i.session.id === dragInfo.id
        : i.type === "group" && i.group.id === dragInfo.id
    );
    setActiveDragItem(item || null);
  };

  // Context menu handlers
  const handleSessionContextMenu = (session: Session, x: number, y: number) => {
    setContextMenu({ type: "session", session, x, y });
  };

  const handleGroupContextMenu = (group: Group, x: number, y: number) => {
    setContextMenu({ type: "group", group, x, y });
  };

  // Build context menu items for a session
  const getSessionMenuItems = (session: Session): ContextMenuItem[] => {
    const isRunning = session.status === "running";
    const hasClaudeSession = !!session.claude_session_id;

    return [
      {
        label: "Restart",
        icon: MenuIcons.restart,
        onClick: () => appStore.restartSession(session.id),
        disabled: false,
      },
      {
        label: isRunning ? "Stop" : "Start",
        icon: isRunning ? MenuIcons.stop : MenuIcons.play,
        onClick: () => {
          if (isRunning) {
            appStore.stopSession(session.id);
          } else {
            appStore.restartSession(session.id);
          }
        },
      },
      { label: "", separator: true, onClick: () => {} },
      {
        label: "Fork",
        icon: MenuIcons.fork,
        onClick: () => appStore.forkSession(session.id),
        disabled: !hasClaudeSession,
      },
      {
        label: "Edit",
        icon: MenuIcons.edit,
        onClick: () => setEditingSession(session),
      },
      { label: "", separator: true, onClick: () => {} },
      {
        label: "Delete",
        icon: MenuIcons.delete,
        onClick: () => {
          if (confirm(`Delete session "${session.name}"?`)) {
            appStore.deleteSession(session.id);
          }
        },
        danger: true,
      },
    ];
  };

  // Build context menu items for a group
  const getGroupMenuItems = (group: Group): ContextMenuItem[] => {
    return [
      {
        label: "Edit",
        icon: MenuIcons.edit,
        onClick: () => setEditingGroup(group),
      },
      { label: "", separator: true, onClick: () => {} },
      {
        label: "Delete",
        icon: MenuIcons.delete,
        onClick: () => {
          if (confirm(`Delete group "${group.name}"? Sessions will be moved to root.`)) {
            appStore.deleteGroup(group.id);
          }
        },
        danger: true,
      },
    ];
  };

  // Keyboard navigation handler
  const handleKeyDown = (e: KeyboardEvent) => {
    // Only handle when sidebar has focus and no dialog is open
    if (!hasSidebarFocus()) return;
    if (editingSession() || editingGroup() || isNewSessionOpen() || isNewGroupOpen() || isSettingsOpen()) return;

    const items = navItems();
    const currentIndex = focusedIndex();

    switch (e.key) {
      case "ArrowDown":
      case "j": // Vim-style
        e.preventDefault();
        if (items.length > 0) {
          const nextIndex = currentIndex < items.length - 1 ? currentIndex + 1 : 0;
          setFocusedIndex(nextIndex);
        }
        break;

      case "ArrowUp":
      case "k": // Vim-style
        e.preventDefault();
        if (items.length > 0) {
          const prevIndex = currentIndex > 0 ? currentIndex - 1 : items.length - 1;
          setFocusedIndex(prevIndex);
        }
        break;

      case "Enter":
      case " ": // Space
        e.preventDefault();
        if (currentIndex >= 0 && currentIndex < items.length) {
          const item = items[currentIndex];
          if (item.type === "session") {
            appStore.setSelectedSessionId(item.session.id);
          } else {
            appStore.toggleGroupCollapse(item.group.id);
          }
        }
        break;

      case "ArrowRight":
      case "l": // Vim-style
        e.preventDefault();
        if (currentIndex >= 0 && currentIndex < items.length) {
          const item = items[currentIndex];
          if (item.type === "group" && item.group.collapsed) {
            appStore.toggleGroupCollapse(item.group.id);
          }
        }
        break;

      case "ArrowLeft":
      case "h": // Vim-style
        e.preventDefault();
        if (currentIndex >= 0 && currentIndex < items.length) {
          const item = items[currentIndex];
          if (item.type === "group" && !item.group.collapsed) {
            appStore.toggleGroupCollapse(item.group.id);
          }
        }
        break;

      case "Home":
        e.preventDefault();
        if (items.length > 0) {
          setFocusedIndex(0);
        }
        break;

      case "End":
        e.preventDefault();
        if (items.length > 0) {
          setFocusedIndex(items.length - 1);
        }
        break;

      case "Escape":
        e.preventDefault();
        setFocusedIndex(-1);
        listContainerRef?.blur();
        setHasSidebarFocus(false);
        break;

      case "/":
        // Focus search
        e.preventDefault();
        searchInputRef?.focus();
        break;
    }
  };

  // Set up global keyboard listener when sidebar is focused
  onMount(() => {
    const handler = (e: KeyboardEvent) => handleKeyDown(e);
    window.addEventListener("keydown", handler);
    onCleanup(() => window.removeEventListener("keydown", handler));
  });

  return (
    <div class="w-64 bg-gray-800 border-r border-gray-700 flex flex-col h-full">
      {/* Header */}
      <div class="px-3 py-2 border-b border-gray-700 flex items-center justify-between">
        <h1 class="text-sm font-semibold text-white">Claude Master</h1>
        <Show when={appStore.isConnected()}>
          <span class="w-2 h-2 rounded-full bg-green-500" title="Connected" />
        </Show>
        <Show when={!appStore.isConnected()}>
          <button
            class="px-2 py-0.5 text-xs bg-indigo-600 hover:bg-indigo-700 rounded text-white transition-colors"
            onClick={() => appStore.connectToDaemon()}
          >
            Connect
          </button>
        </Show>
      </div>

      {/* Search box */}
      <div class="px-2 py-1.5 border-b border-gray-700">
        <div class="relative">
          <svg
            class="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-500"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
            />
          </svg>
          <input
            ref={searchInputRef}
            type="text"
            placeholder="Search... (/)"
            value={searchQuery()}
            onInput={(e) => {
              setSearchQuery(e.currentTarget.value);
              setFocusedIndex(0);
            }}
            onFocus={() => setHasSidebarFocus(false)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown" && navItems().length > 0) {
                e.preventDefault();
                setHasSidebarFocus(true);
                setFocusedIndex(0);
                listContainerRef?.focus();
              } else if (e.key === "Escape") {
                e.currentTarget.blur();
                setSearchQuery("");
              }
            }}
            class="w-full pl-7 pr-7 py-1 text-xs bg-gray-700 border border-gray-600 rounded text-white placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-indigo-500 focus:border-indigo-500"
          />
          <Show when={searchQuery()}>
            <button
              class="absolute right-2 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-300"
              onClick={() => setSearchQuery("")}
              tabIndex={-1}
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
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
            </button>
          </Show>
        </div>
      </div>

      {/* Session/Group list */}
      <div
        ref={listContainerRef}
        class={`flex-1 overflow-y-auto px-1.5 py-1 focus:outline-none ${
          hasSidebarFocus() ? "bg-gray-800/50" : ""
        }`}
        tabIndex={0}
        onFocus={() => {
          setHasSidebarFocus(true);
          if (focusedIndex() < 0 && navItems().length > 0) {
            setFocusedIndex(0);
          }
        }}
        onBlur={(e) => {
          // Only lose focus if focus is leaving the sidebar entirely
          if (!e.currentTarget.contains(e.relatedTarget as Node)) {
            setHasSidebarFocus(false);
          }
        }}
      >
        {/* Keyboard hint - always visible */}
        <Show when={navItems().length > 0}>
          <div class="text-[10px] text-gray-500 mb-1 px-0.5">
            ↑↓ nav • Enter select • ←→ toggle
          </div>
        </Show>

        {/* Search results mode */}
        <Show when={filteredSessions()}>
          <div class="text-[10px] text-gray-500 mb-1 px-0.5">
            {filteredSessions()!.length} result(s)
          </div>
          <For each={filteredSessions()!}>
            {(session, index) => (
              <SessionItem
                session={session}
                depth={0}
                isFocused={hasSidebarFocus() && focusedIndex() === index()}
                onFocus={() => setFocusedIndex(index())}
                onContextMenu={handleSessionContextMenu}
                listContainerRef={listContainerRef}
              />
            )}
          </For>
          <Show when={filteredSessions()!.length === 0}>
            <div class="text-gray-500 text-xs text-center py-2">
              No matches
            </div>
          </Show>
        </Show>

        {/* Normal tree mode with drag and drop */}
        <Show when={!filteredSessions()}>
          <DragDropProvider
            onDragStart={handleDragStart}
            onDragEnd={handleDragEnd}
            collisionDetector={closestCenter}
          >
            <DragDropSensors />
            <SortableProvider ids={navItems().map((item) =>
              item.type === "session"
                ? makeDragId("session", item.session.id)
                : makeDragId("group", item.group.id)
            )}>
              <For each={navItems()}>
                {(item, index) => (
                  <Show
                    when={item.type === "group"}
                    fallback={
                      <SortableSession
                        session={(item as NavItem & { type: "session" }).session}
                        depth={item.depth}
                        groupId={(item as NavItem & { type: "session" }).groupId}
                        isFocused={hasSidebarFocus() && focusedIndex() === index()}
                        onFocus={() => setFocusedIndex(index())}
                        onContextMenu={handleSessionContextMenu}
                        listContainerRef={listContainerRef}
                      />
                    }
                  >
                    <SortableGroup
                      group={(item as NavItem & { type: "group" }).group}
                      depth={item.depth}
                      parentId={(item as NavItem & { type: "group" }).parentId}
                      isFocused={hasSidebarFocus() && focusedIndex() === index()}
                      onEditGroup={handleEditGroup}
                      onFocus={() => setFocusedIndex(index())}
                      onToggle={() => appStore.toggleGroupCollapse((item as NavItem & { type: "group" }).group.id)}
                      onContextMenu={handleGroupContextMenu}
                      listContainerRef={listContainerRef}
                    />
                  </Show>
                )}
              </For>
            </SortableProvider>

            {/* Drag overlay - shows what's being dragged */}
            <DragOverlay>
              {activeDragItem() && (
                <div class="bg-gray-700 rounded px-2 py-1 text-sm text-white shadow-lg border border-indigo-500">
                  {activeDragItem()!.type === "session"
                    ? (activeDragItem() as NavItem & { type: "session" }).session.name
                    : (activeDragItem() as NavItem & { type: "group" }).group.name}
                </div>
              )}
            </DragOverlay>
          </DragDropProvider>

          {/* Empty state */}
          <Show when={navItems().length === 0}>
            <div class="text-gray-500 text-xs text-center py-4">
              No sessions
            </div>
          </Show>
        </Show>
      </div>

      {/* Footer actions */}
      <div class="px-2 py-1.5 border-t border-gray-700">
        <div class="flex gap-1.5">
          <button
            class="flex-1 flex items-center justify-center gap-1 px-2 py-1 text-xs bg-indigo-600 hover:bg-indigo-700 rounded text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            disabled={!appStore.isConnected()}
            onClick={() => setIsNewSessionOpen(true)}
            title="New Session"
          >
            <svg
              class="w-3 h-3"
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
            Session
          </button>
          <button
            class="flex-1 flex items-center justify-center gap-1 px-2 py-1 text-xs bg-gray-700 hover:bg-gray-600 rounded text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            disabled={!appStore.isConnected()}
            onClick={() => setIsNewGroupOpen(true)}
            title="New Group"
          >
            <svg
              class="w-3 h-3"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
              />
            </svg>
            Group
          </button>
          <button
            class="p-1 text-gray-400 hover:text-white hover:bg-gray-700 rounded transition-colors"
            onClick={() => setIsSettingsOpen(true)}
            title="Settings"
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
          </button>
        </div>
      </div>

      {/* New Session Dialog */}
      <NewSessionDialog
        isOpen={isNewSessionOpen()}
        onClose={() => setIsNewSessionOpen(false)}
      />

      {/* New Group Dialog */}
      <NewGroupDialog
        isOpen={isNewGroupOpen()}
        onClose={() => setIsNewGroupOpen(false)}
      />

      {/* Edit Session Dialog */}
      <EditSessionDialog
        isOpen={editingSession() !== null}
        onClose={() => setEditingSession(null)}
        session={editingSession()}
      />

      {/* Edit Group Dialog */}
      <EditGroupDialog
        isOpen={editingGroup() !== null}
        onClose={() => setEditingGroup(null)}
        group={editingGroup()}
      />

      {/* Settings Modal */}
      <SettingsModal
        isOpen={isSettingsOpen()}
        onClose={() => setIsSettingsOpen(false)}
      />

      {/* Context Menu */}
      <Show when={contextMenu()}>
        {(menu) => {
          const menuData = menu();
          const items = menuData.type === "session"
            ? getSessionMenuItems((menuData as { type: "session"; session: Session; x: number; y: number }).session)
            : getGroupMenuItems((menuData as { type: "group"; group: Group; x: number; y: number }).group);
          return (
            <ContextMenu
              x={menuData.x}
              y={menuData.y}
              items={items}
              onClose={() => setContextMenu(null)}
            />
          );
        }}
      </Show>
    </div>
  );
}
