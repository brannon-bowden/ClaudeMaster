// Application state store using SolidJS primitives

import { createSignal } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Session, Group, GroupNode, PtyOutputData, StatusChangedData, ConnectionStateData } from "../types";
import { terminalStore } from "./terminalStore";
import { showToast } from "../components/Toast";

// State signals
const [sessions, setSessions] = createSignal<Session[]>([]);
const [groups, setGroups] = createSignal<Group[]>([]);
const [selectedSessionId, setSelectedSessionId] = createSignal<string | null>(
  null
);
const [isConnected, setIsConnected] = createSignal(false);
const [connectionError, setConnectionError] = createSignal<string | null>(null);

// Computed: build tree structure from flat groups
function buildGroupTree(
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

// Actions
// Set up event listeners from Tauri
async function setupEventListeners() {
  // Listen for PTY output
  await listen<PtyOutputData>("pty.output", (event) => {
    terminalStore.writeBase64ToTerminal(event.payload.session_id, event.payload.output);
  });

  // Listen for status changes
  await listen<StatusChangedData>("session.status_changed", (event) => {
    setSessions((prev) =>
      prev.map((s) =>
        s.id === event.payload.session_id
          ? { ...s, status: event.payload.status }
          : s
      )
    );
  });

  // Listen for session created
  await listen<Session>("session.created", (event) => {
    setSessions((prev) => {
      // Avoid duplicates
      if (prev.find((s) => s.id === event.payload.id)) {
        return prev;
      }
      return [...prev, event.payload];
    });
  });

  // Listen for session deleted
  await listen<{ session_id: string }>("session.deleted", (event) => {
    setSessions((prev) => prev.filter((s) => s.id !== event.payload.session_id));
    if (selectedSessionId() === event.payload.session_id) {
      setSelectedSessionId(null);
    }
  });

  // Listen for group created
  await listen<Group>("group.created", (event) => {
    setGroups((prev) => {
      if (prev.find((g) => g.id === event.payload.id)) {
        return prev;
      }
      return [...prev, event.payload];
    });
  });

  // Listen for group deleted
  await listen<{ group_id: string }>("group.deleted", (event) => {
    setGroups((prev) => prev.filter((g) => g.id !== event.payload.group_id));
  });

  // Listen for connection state changes from event listener
  await listen<ConnectionStateData>("daemon.connection_state", (event) => {
    const wasConnected = isConnected();
    setIsConnected(event.payload.connected);

    if (event.payload.connected) {
      // Successfully connected/reconnected
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
    } else {
      // Disconnected
      if (wasConnected) {
        const errorMsg = event.payload.error || "Disconnected from daemon";
        setConnectionError(errorMsg);
        showToast(errorMsg, "error");
      }
    }
  });
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
    const [sessionList, groupList] = await Promise.all([
      invoke<Session[]>("list_sessions"),
      invoke<Group[]>("list_groups"),
    ]);
    setSessions(sessionList);
    setGroups(groupList);
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
    setSessions((prev) => [...prev, session]);
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
    setSessions((prev) =>
      prev.map((s) => (s.id === sessionId ? { ...s, status: "Stopped" } : s))
    );
  } catch (e) {
    console.error("Failed to stop session:", e);
    throw e;
  }
}

async function deleteSession(sessionId: string) {
  try {
    await invoke("delete_session", { sessionId });
    setSessions((prev) => prev.filter((s) => s.id !== sessionId));
    if (selectedSessionId() === sessionId) {
      setSelectedSessionId(null);
    }
  } catch (e) {
    console.error("Failed to delete session:", e);
    throw e;
  }
}

async function forkSession(
  sessionId: string,
  newName?: string,
  groupId?: string
) {
  try {
    const session = await invoke<Session>("fork_session", {
      sessionId,
      newName: newName || null,
      groupId: groupId || null,
    });
    setSessions((prev) => [...prev, session]);
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
    setGroups((prev) => [...prev, group]);
    return group;
  } catch (e) {
    console.error("Failed to create group:", e);
    throw e;
  }
}

async function deleteGroup(groupId: string) {
  try {
    await invoke("delete_group", { groupId });
    setGroups((prev) => prev.filter((g) => g.id !== groupId));
    // Sessions in this group are now orphaned - refresh to get updated data
    await refreshData();
  } catch (e) {
    console.error("Failed to delete group:", e);
    throw e;
  }
}

function toggleGroupCollapse(groupId: string) {
  setGroups((prev) =>
    prev.map((g) => (g.id === groupId ? { ...g, collapsed: !g.collapsed } : g))
  );
}

// Export store
export const appStore = {
  // State (read-only)
  sessions,
  groups,
  selectedSessionId,
  isConnected,
  connectionError,

  // Computed
  get groupTree() {
    return buildGroupTree(groups(), sessions());
  },

  get selectedSession() {
    const id = selectedSessionId();
    return id ? sessions().find((s) => s.id === id) || null : null;
  },

  // Actions
  connectToDaemon,
  refreshData,
  setSelectedSessionId,
  createSession,
  stopSession,
  deleteSession,
  forkSession,
  createGroup,
  deleteGroup,
  toggleGroupCollapse,
};
