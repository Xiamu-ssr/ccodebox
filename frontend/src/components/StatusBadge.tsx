"use client";

const STATUS_STYLES: Record<
  string,
  { bg: string; text: string; dot: string; animate?: boolean }
> = {
  pending: {
    bg: "bg-status-pending/10",
    text: "text-status-pending",
    dot: "bg-status-pending",
  },
  running: {
    bg: "bg-status-running/10",
    text: "text-status-running",
    dot: "bg-status-running",
    animate: true,
  },
  success: {
    bg: "bg-status-success/10",
    text: "text-status-success",
    dot: "bg-status-success",
  },
  failed: {
    bg: "bg-status-failed/10",
    text: "text-status-failed",
    dot: "bg-status-failed",
  },
  cancelled: {
    bg: "bg-status-cancelled/10",
    text: "text-status-cancelled",
    dot: "bg-status-cancelled",
  },
};

const DEFAULT_STYLE = {
  bg: "bg-gray-500/10",
  text: "text-gray-400",
  dot: "bg-gray-400",
};

export default function StatusBadge({ status }: { status: string }) {
  const style = STATUS_STYLES[status] ?? DEFAULT_STYLE;

  return (
    <span
      className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium ${style.bg} ${style.text}`}
    >
      <span
        className={`w-1.5 h-1.5 rounded-full ${style.dot} ${
          style.animate ? "animate-pulse" : ""
        }`}
      />
      {status}
    </span>
  );
}
