"use client";

import Link from "next/link";
import type { Task } from "@/lib/generated/Task";
import StatusBadge from "./StatusBadge";

function formatTime(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString();
}

export default function TaskCard({ task }: { task: Task }) {
  return (
    <Link href={`/tasks/${task.id}`}>
      <div className="group border border-border rounded-lg p-4 bg-bg-surface hover:bg-bg-elevated hover:border-primary/50 transition-all cursor-pointer">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0 flex-1">
            <h3 className="text-sm font-semibold text-text-primary truncate group-hover:text-primary transition-colors">
              {task.title}
            </h3>
            <p className="text-xs text-text-muted mt-1 line-clamp-2">
              {task.prompt}
            </p>
          </div>
          <StatusBadge status={task.status} />
        </div>
        <div className="flex items-center gap-3 mt-3 text-xs text-text-secondary">
          <span className="font-mono">
            {task.agent_type === "claude_code" ? "Claude Code" : task.agent_type}
          </span>
          <span className="text-text-muted">|</span>
          <span className="font-mono">{task.model}</span>
          <span className="flex-1" />
          <span className="text-text-muted">{formatTime(task.created_at)}</span>
        </div>
      </div>
    </Link>
  );
}
