// Types matching the Rust shared crate

// Must match Rust enum with #[serde(rename_all = "lowercase")]
export type SessionStatus =
  | "stopped"
  | "running"
  | "waiting"
  | "error"
  | "idle";

export interface Session {
  id: string;
  name: string;
  working_dir: string;
  group_id: string | null;
  status: SessionStatus;
  pid: number | null;
  claude_session_id: string | null;
  created_at: string;
  last_activity: string;
  order: number;
}

export interface Group {
  id: string;
  name: string;
  parent_id: string | null;
  collapsed: boolean;
  order: number;
}

// Event types from daemon
export interface PtyOutputData {
  session_id: string;
  output: string; // base64 encoded
}

export interface StatusChangedData {
  session_id: string;
  status: SessionStatus;
}

export interface ConnectionStateData {
  connected: boolean;
  error: string | null;
}

// Tree structure for rendering sidebar
export interface GroupNode extends Group {
  children: GroupNode[];
  sessions: Session[];
}
