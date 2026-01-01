// Application state store using SolidJS primitives

import { createSignal } from "solid-js";
import { createStore, produce, reconcile } from "solid-js/store";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Session, Group, GroupNode, PtyOutputData, StatusChangedData, ConnectionStateData } from "../types";
import { terminalStore } from "./terminalStore";
import { showToast } from "../components/Toast";

// Use createStore for sessions to enable fine-grained updates
// This preserves object references when updating individual session properties,
// which prevents SolidJS's <For> from recreating Terminal components on status changes
const [sessions, setSessions] = createStore<Session[]>([]);
const [groups, setGroups] = createStore<Group[]>([]);
const [selectedSessionId, setSelectedSessionId] = createSignal<string | null>(
  null
);
const [isConnected, setIsConnected] = createSignal(false);
const [connectionError, setConnectionError] = createSignal<string | null>(null);

// Computed: build tree structure from flat groups
// Exported for use in components that need to access stores reactively
export function buildGroupTree(
  groups: Group[],
  sessions: Session[]
): { roots: GroupNode[]; orphanSessions: Session[] } {
  const groupMap = new Map<string, GroupNode>();

  // Initialize all groups as nodes
  for (const group of groups) {
    groupMap.set(group.id, {
      ...group,
      children: [],
      sessions: [],
    });
  }

  // Assign sessions to their groups
  const orphanSessions: Session[] = [];
  for (const session of sessions) {
    if (session.group_id && groupMap.has(session.group_id)) {
      groupMap.get(session.group_id)!.sessions.push(session);
    } else {
      orphanSessions.push(session);
    }
  }

  // Build parent-child relationships
  const roots: GroupNode[] = [];
  for (const node of groupMap.values()) {
    if (node.parent_id && groupMap.has(node.parent_id)) {
      groupMap.get(node.parent_id)!.children.push(node);
    } else {
      roots.push(node);
    }
  }

  // Sort by order
  const sortByOrder = (a: { order: number }, b: { order: number }) =>
    a.order - b.order;
  roots.sort(sortByOrder);
  for (const node of groupMap.values()) {
    node.children.sort(sortByOrder);
    node.sessions.sort(sortByOrder);
  }
  orphanSessions.sort(sortByOrder);

  return { roots, orphanSessions };
}

// Reconnection state (managed by event listener on Rust side)
let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;

// Track if event listeners have been set up (prevent duplicates from HMR)
let eventListenersSetup = false;
// Store unlisten functions for cleanup
let unlistenFunctions: Array<() => void> = [];

// Actions
// Set up event listeners from Tauri
async function setupEventListeners() {
  // Prevent duplicate listeners (especially during HMR)
  if (eventListenersSetup) {
    console.log("[AppStore] Event listeners already set up, skipping");
    return;
  }
  eventListenersSetup = true;
  console.log("[AppStore] Setting up event listeners");

  // Listen for PTY output
  const unlistenPty = await listen<PtyOutputData>("pty:output", (event) => {
    terminalStore.writeBase64ToTerminal(event.payload.session_id, event.payload.output);
  });
  unlistenFunctions.push(unlistenPty);

  // Listen for status changes - use fine-grained store update to preserve object reference
  const unlistenStatus = await listen<StatusChangedData>("session:status_changed", (event) => {
    const index = sessions.findIndex((s) => s.id === event.payload.session_id);
    if (index !== -1) {
      // Update only the status property, preserving the object reference
      setSessions(index, "status", event.payload.status);
    }
  });
  unlistenFunctions.push(unlistenStatus);

  // Listen for session created
  const unlistenSessionCreated = await listen<Session>("session:created", (event) => {
    // Avoid duplicates
    if (!sessions.find((s) => s.id === event.payload.id)) {
      setSessions(produce((draft) => draft.push(event.payload)));
    }
  });
  unlistenFunctions.push(unlistenSessionCreated);

  // Listen for session deleted
  const unlistenSessionDeleted = await listen<{ session_id: string }>("session:deleted", (event) => {
    const index = sessions.findIndex((s) => s.id === event.payload.session_id);
    if (index !== -1) {
      setSessions(produce((draft) => draft.splice(index, 1)));
    }
    if (selectedSessionId() === event.payload.session_id) {
      setSelectedSessionId(null);
    }
  });
  unlistenFunctions.push(unlistenSessionDeleted);

  // Listen for group created
  const unlistenGroupCreated = await listen<Group>("group:created", (event) => {
    if (!groups.find((g) => g.id === event.payload.id)) {
      setGroups(produce((draft) => draft.push(event.payload)));
    }
  });
  unlistenFunctions.push(unlistenGroupCreated);

  // Listen for group deleted
  const unlistenGroupDeleted = await listen<{ group_id: string }>("group:deleted", (event) => {
    const index = groups.findIndex((g) => g.id === event.payload.group_id);
    if (index !== -1) {
      setGroups(produce((draft) => draft.splice(index, 1)));
    }
  });
  unlistenFunctions.push(unlistenGroupDeleted);

  // Listen for session updated - use reconcile to update while preserving reference if possible
  const unlistenSessionUpdated = await listen<Session>("session:updated", (event) => {
    const index = sessions.findIndex((s) => s.id === event.payload.id);
    if (index !== -1) {
      setSessions(index, reconcile(event.payload));
    }
  });
  unlistenFunctions.push(unlistenSessionUpdated);

  // Listen for group updated
  const unlistenGroupUpdated = await listen<Group>("group:updated", (event) => {
    const index = groups.findIndex((g) => g.id === event.payload.id);
    if (index !== -1) {
      setGroups(index, reconcile(event.payload));
    }
  });
  unlistenFunctions.push(unlistenGroupUpdated);

  // Listen for connection state changes from event listener
  const unlistenConnectionState = await listen<ConnectionStateData>("daemon:connection_state", async (event) => {
    const wasConnected = isConnected();

    if (event.payload.connected) {
      // Event listener connected - ensure command client is also connected
      try {
        // Try to reconnect the command IPC client if needed
        await invoke("connect_daemon");
        setIsConnected(true);
        setConnectionError(null);
        if (reconnectTimeout) {
          clearTimeout(reconnectTimeout);
          reconnectTimeout = null;
        }
        if (!wasConnected) {
          showToast("Connected to daemon", "success");
          // Refresh data on reconnection
          refreshData().catch(console.error);
        }
      } catch (e) {
        // Failed to connect command client
        console.error("Failed to connect command client:", e);
        setConnectionError(String(e));
        setIsConnected(false);
      }
    } else {
      // Disconnected
      setIsConnected(false);
      if (wasConnected) {
        const errorMsg = event.payload.error || "Disconnected from daemon";
        setConnectionError(errorMsg);
        showToast(errorMsg, "error");
      }
    }
  });
  unlistenFunctions.push(unlistenConnectionState);
}

async function connectToDaemon() {
  try {
    setConnectionError(null);
    await invoke("connect_daemon");
    setIsConnected(true);
    await setupEventListeners();
    await refreshData();
  } catch (e) {
    setConnectionError(String(e));
    setIsConnected(false);
  }
}

async function refreshData() {
  try {
    console.log("[AppStore] Refreshing data...");
    const [sessionList, groupList] = await Promise.all([
      invoke<Session[]>("list_sessions"),
      invoke<Group[]>("list_groups"),
    ]);
    console.log("[AppStore] Received sessions:", sessionList.length, "groups:", groupList.length);
    // Use reconcile to intelligently update while preserving references where possible
    setSessions(reconcile(sessionList));
    setGroups(reconcile(groupList));
    console.log("[AppStore] Stores updated - sessions:", sessions.length, "groups:", groups.length);
  } catch (e) {
    console.error("Failed to refresh data:", e);
  }
}

async function createSession(name: string, dir: string, groupId?: string) {
  try {
    const session = await invoke<Session>("create_session", {
      name,
      dir,
      groupId: groupId || null,
    });
    // Don't add to store here - the session:created event will do it
    // This prevents duplicate entries
    setSelectedSessionId(session.id);
    return session;
  } catch (e) {
    console.error("Failed to create session:", e);
    throw e;
  }
}

async function stopSession(sessionId: string) {
  try {
    await invoke("stop_session", { sessionId });
    // Clear the terminal screen and buffers
    terminalStore.clearTerminal(sessionId);
    const index = sessions.findIndex((s) => s.id === sessionId);
    if (index !== -1) {
      setSessions(index, "status", "stopped");
    }
  } catch (e) {
    console.error("Failed to stop session:", e);
    throw e;
  }
}

async function deleteSession(sessionId: string) {
  try {
    await invoke("delete_session", { sessionId });
    const index = sessions.findIndex((s) => s.id === sessionId);
    if (index !== -1) {
      setSessions(produce((draft) => draft.splice(index, 1)));
    }
    if (selectedSessionId() === sessionId) {
      setSelectedSessionId(null);
    }
  } catch (e) {
    console.error("Failed to delete session:", e);
    throw e;
  }
}

async function restartSession(sessionId: string, rows: number = 24, cols: number = 80) {
  try {
    console.log(`[AppStore] Restarting session ${sessionId} with size ${cols}x${rows}`);
    // Clear the terminal screen and buffers before restart
    terminalStore.clearTerminal(sessionId);
    const session = await invoke<Session>("restart_session", {
      sessionId,
      rows,
      cols,
    });
    const index = sessions.findIndex((s) => s.id === sessionId);
    if (index !== -1) {
      setSessions(index, reconcile(session));
    }
    return session;
  } catch (e) {
    console.error("Failed to restart session:", e);
    throw e;
  }
}

async function forkSession(
  sessionId: string,
  newName?: string,
  groupId?: string,
  rows: number = 24,
  cols: number = 80
) {
  try {
    console.log(`[AppStore] Forking session ${sessionId} with size ${cols}x${rows}`);
    const session = await invoke<Session>("fork_session", {
      sessionId,
      newName: newName || null,
      groupId: groupId || null,
      rows,
      cols,
    });
    // Don't add to store here - the session:created event will do it
    setSelectedSessionId(session.id);
    return session;
  } catch (e) {
    console.error("Failed to fork session:", e);
    throw e;
  }
}

async function createGroup(name: string, parentId?: string) {
  try {
    const group = await invoke<Group>("create_group", {
      name,
      parentId: parentId || null,
    });
    // Don't add to store here - the group:created event will do it
    return group;
  } catch (e) {
    console.error("Failed to create group:", e);
    throw e;
  }
}

async function deleteGroup(groupId: string) {
  try {
    await invoke("delete_group", { groupId });
    const index = groups.findIndex((g) => g.id === groupId);
    if (index !== -1) {
      setGroups(produce((draft) => draft.splice(index, 1)));
    }
    // Sessions in this group are now orphaned - refresh to get updated data
    await refreshData();
  } catch (e) {
    console.error("Failed to delete group:", e);
    throw e;
  }
}

async function updateSession(
  sessionId: string,
  name?: string,
  groupId?: string | null // undefined = don't change, null = remove from group, string = set group
) {
  try {
    // Convert: undefined = don't pass (backend won't change), null = pass "" (backend removes), string = pass as-is
    const groupIdParam = groupId === undefined ? undefined : (groupId === null ? "" : groupId);
    const session = await invoke<Session>("update_session", {
      sessionId,
      name: name || null,
      groupId: groupIdParam,
    });
    const index = sessions.findIndex((s) => s.id === sessionId);
    if (index !== -1) {
      setSessions(index, reconcile(session));
    }
    return session;
  } catch (e) {
    console.error("Failed to update session:", e);
    throw e;
  }
}

async function updateGroup(
  groupId: string,
  name?: string,
  parentId?: string | null // undefined = don't change, null = make root, string = set parent
) {
  try {
    // Convert: undefined = don't pass (backend won't change), null = pass "" (backend makes root), string = pass as-is
    const parentIdParam = parentId === undefined ? undefined : (parentId === null ? "" : parentId);
    const group = await invoke<Group>("update_group", {
      groupId,
      name: name || null,
      parentId: parentIdParam,
    });
    const index = groups.findIndex((g) => g.id === groupId);
    if (index !== -1) {
      setGroups(index, reconcile(group));
    }
    return group;
  } catch (e) {
    console.error("Failed to update group:", e);
    throw e;
  }
}

function toggleGroupCollapse(groupId: string) {
  const index = groups.findIndex((g) => g.id === groupId);
  if (index !== -1) {
    setGroups(index, "collapsed", (prev) => !prev);
  }
}

async function reorderSession(
  sessionId: string,
  groupId: string | null, // null = root level
  afterSessionId: string | null // null = insert at beginning
) {
  try {
    const session = await invoke<Session>("reorder_session", {
      sessionId,
      groupId: groupId || null,
      afterSessionId: afterSessionId || null,
    });
    // Refresh all data since multiple sessions may have had their order updated
    await refreshData();
    return session;
  } catch (e) {
    console.error("Failed to reorder session:", e);
    throw e;
  }
}

async function reorderGroup(
  groupId: string,
  parentId: string | null, // null = root level
  afterGroupId: string | null // null = insert at beginning
) {
  try {
    const group = await invoke<Group>("reorder_group", {
      groupId,
      parentId: parentId || null,
      afterGroupId: afterGroupId || null,
    });
    // Refresh all data since multiple groups may have had their order updated
    await refreshData();
    return group;
  } catch (e) {
    console.error("Failed to reorder group:", e);
    throw e;
  }
}

// Export store
// Note: sessions and groups are createStore arrays, accessed as functions for consistency
// with the rest of the codebase that expects signals
export const appStore = {
  // State (read-only) - wrap stores as accessors for backward compatibility
  // Components call appStore.sessions() expecting a signal-like accessor
  sessions: () => sessions,
  groups: () => groups,
  selectedSessionId,
  isConnected,
  connectionError,

  // Computed
  get groupTree() {
    return buildGroupTree(groups, sessions);
  },

  get selectedSession() {
    const id = selectedSessionId();
    return id ? sessions.find((s) => s.id === id) || null : null;
  },

  // Actions
  connectToDaemon,
  refreshData,
  setSelectedSessionId,
  createSession,
  stopSession,
  deleteSession,
  restartSession,
  forkSession,
  updateSession,
  reorderSession,
  createGroup,
  deleteGroup,
  updateGroup,
  reorderGroup,
  toggleGroupCollapse,
};
