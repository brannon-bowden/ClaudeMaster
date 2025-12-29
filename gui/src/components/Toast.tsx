// Toast notification component for displaying errors and messages

import { For, createSignal, onCleanup } from "solid-js";

export type ToastType = "success" | "error" | "warning" | "info";

interface Toast {
  id: number;
  type: ToastType;
  message: string;
  duration: number;
}

// Global toast state
const [toasts, setToasts] = createSignal<Toast[]>([]);
let nextId = 1;

// Toast colors by type
const toastColors: Record<ToastType, string> = {
  success: "bg-green-600",
  error: "bg-red-600",
  warning: "bg-yellow-600",
  info: "bg-blue-600",
};

// Icons by type
const ToastIcon = (props: { type: ToastType }) => {
  switch (props.type) {
    case "success":
      return (
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
        </svg>
      );
    case "error":
      return (
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      );
    case "warning":
      return (
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
        </svg>
      );
    case "info":
    default:
      return (
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
      );
  }
};

// Add a toast
export function showToast(message: string, type: ToastType = "info", duration: number = 5000) {
  const id = nextId++;
  const toast: Toast = { id, type, message, duration };

  setToasts((prev) => [...prev, toast]);

  // Auto-remove after duration
  if (duration > 0) {
    setTimeout(() => {
      removeToast(id);
    }, duration);
  }

  return id;
}

// Remove a toast by id
export function removeToast(id: number) {
  setToasts((prev) => prev.filter((t) => t.id !== id));
}

// Toast container component
export function ToastContainer() {
  return (
    <div class="fixed bottom-4 right-4 z-50 flex flex-col gap-2 max-w-sm">
      <For each={toasts()}>
        {(toast) => (
          <div
            class={`${toastColors[toast.type]} text-white px-4 py-3 rounded-lg shadow-lg flex items-center gap-3 animate-slide-in`}
            role="alert"
          >
            <ToastIcon type={toast.type} />
            <span class="flex-1 text-sm">{toast.message}</span>
            <button
              class="opacity-70 hover:opacity-100 transition-opacity"
              onClick={() => removeToast(toast.id)}
              aria-label="Close"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        )}
      </For>
    </div>
  );
}

// Hook for toast with auto-cleanup
export function useToast() {
  const toastIds: number[] = [];

  onCleanup(() => {
    toastIds.forEach(removeToast);
  });

  return {
    show: (message: string, type: ToastType = "info", duration: number = 5000) => {
      const id = showToast(message, type, duration);
      toastIds.push(id);
      return id;
    },
    remove: removeToast,
  };
}
