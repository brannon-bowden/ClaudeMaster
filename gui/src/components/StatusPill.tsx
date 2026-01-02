// Status pill component - displays session status as a readable text label
// Replaces the tiny colored dots with clear, accessible status indicators

import type { SessionStatus } from "../types";

// Status-specific styling with background and text colors
const styles: Record<SessionStatus, { bg: string; text: string }> = {
  waiting: { bg: "bg-amber-900/50", text: "text-amber-400" },
  running: { bg: "bg-blue-900/50", text: "text-blue-400" },
  idle: { bg: "bg-gray-700", text: "text-gray-400" },
  error: { bg: "bg-red-900/50", text: "text-red-400" },
  stopped: { bg: "bg-gray-800", text: "text-gray-500" },
};

interface StatusPillProps {
  status: SessionStatus;
}

export function StatusPill(props: StatusPillProps) {
  const style = () => styles[props.status];

  return (
    <span
      class={`text-[10px] px-1.5 py-0.5 rounded uppercase font-medium tracking-wide
              flex-shrink-0 ${style().bg} ${style().text}`}
    >
      {props.status}
    </span>
  );
}
