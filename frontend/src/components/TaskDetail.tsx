"use client";

import { useState, useEffect, useCallback } from "react";
import type { Task } from "@/lib/generated/Task";
import { getTask, cancelTask } from "@/lib/api";
import StatusBadge from "./StatusBadge";
import LogViewer from "./LogViewer";
import DiffViewer from "./DiffViewer";
import MarkdownViewer from "./MarkdownViewer";

type Tab = "overview" | "logs" | "diff" | "summary";

function formatTime(iso: string | null): string {
  if (!iso) return "-";
  return new Date(iso).toLocaleString();
}

function duration(start: string | null, end: string | null): string {
  if (!start) return "-";
  const s = new Date(start).getTime();
  const e = end ? new Date(end).getTime() : Date.now();
  const sec = Math.floor((e - s) / 1000);
  if (sec < 60) return `${sec}s`;
  const min = Math.floor(sec / 60);
  return `${min}m ${sec % 60}s`;
}

export default function TaskDetail({ taskId }: { taskId: string }) {
  const [task, setTask] = useState<Task | null>(null);
  const [tab, setTab] = useState<Tab>("overview");
  const [loading, setLoading] = useState(true);
  const [cancelling, setCancelling] = useState(false);

  const fetchTask = useCallback(async () => {
    try {
      const t = await getTask(taskId);
      setTask(t);
    } catch (err) {
      console.error("Failed to fetch task:", err);
    } finally {
      setLoading(false);
    }
  }, [taskId]);

  useEffect(() => {
    fetchTask();
  }, [fetchTask]);

  // Poll for running tasks
  useEffect(() => {
    if (!task || task.status !== "running") return;
    const interval = setInterval(fetchTask, 3000);
    return () => clearInterval(interval);
  }, [task, fetchTask]);

  async function handleCancel() {
    if (!task || cancelling) return;
    setCancelling(true);
    try {
      await cancelTask(task.id);
      fetchTask();
    } catch (err) {
      console.error("Failed to cancel:", err);
    } finally {
      setCancelling(false);
    }
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (!task) {
    return (
      <div className="text-center py-20 text-text-secondary">
        Task not found.
      </div>
    );
  }

  const isRunning = task.status === "running";

  const tabs: { key: Tab; label: string }[] = [
    { key: "overview", label: "Overview" },
    { key: "logs", label: "Logs" },
    { key: "diff", label: "Diff" },
    { key: "summary", label: "Summary" },
  ];

  return (
    <div>
      {/* Header */}
      <div className="flex items-start justify-between gap-4 mb-6">
        <div>
          <div className="flex items-center gap-3">
            <h1 className="text-xl font-bold">{task.title}</h1>
            <StatusBadge status={task.status} />
          </div>
          {/* Timeline */}
          <div className="flex items-center gap-4 mt-2 text-xs text-text-secondary">
            <span>Created: {formatTime(task.created_at)}</span>
            <span>Started: {formatTime(task.started_at)}</span>
            <span>Finished: {formatTime(task.finished_at)}</span>
            <span>Duration: {duration(task.started_at, task.finished_at)}</span>
          </div>
        </div>
        {isRunning && (
          <button
            onClick={handleCancel}
            disabled={cancelling}
            className="bg-status-failed/10 border border-status-failed/30 text-status-failed hover:bg-status-failed/20 px-4 py-2 rounded-md text-sm font-medium transition-colors disabled:opacity-50"
          >
            {cancelling ? "Cancelling..." : "Cancel"}
          </button>
        )}
      </div>

      {/* Error display */}
      {task.error && (
        <div className="bg-status-failed/10 border border-status-failed/30 text-status-failed rounded-md p-3 text-sm mb-4 font-mono">
          {task.error}
        </div>
      )}

      {/* Tabs */}
      <div className="flex gap-1 border-b border-border mb-4">
        {tabs.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={`px-4 py-2 text-sm font-medium transition-colors border-b-2 -mb-px ${
              tab === t.key
                ? "text-primary border-primary"
                : "text-text-secondary border-transparent hover:text-text-primary"
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      {tab === "overview" && <OverviewTab task={task} />}
      {tab === "logs" && <LogViewer taskId={task.id} isRunning={isRunning} />}
      {tab === "diff" && <DiffViewer diff={task.diff_patch} />}
      {tab === "summary" && <MarkdownViewer content={task.summary} />}
    </div>
  );
}

function OverviewTab({ task }: { task: Task }) {
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
      {/* Config card */}
      <div className="bg-bg-surface border border-border rounded-md p-4">
        <h3 className="text-sm font-semibold mb-3 text-text-primary">
          Configuration
        </h3>
        <dl className="space-y-2 text-sm">
          <InfoRow label="Agent" value={task.agent_type === "claude_code" ? "Claude Code" : task.agent_type} />
          <InfoRow label="Model" value={task.model} mono />
          <InfoRow label="Max Rounds" value={String(task.max_rounds)} />
          {task.repo_url && <InfoRow label="Repository" value={task.repo_url} mono />}
          {task.branch && <InfoRow label="Branch" value={task.branch} mono />}
        </dl>
      </div>

      {/* Prompt card */}
      <div className="bg-bg-surface border border-border rounded-md p-4">
        <h3 className="text-sm font-semibold mb-3 text-text-primary">
          Prompt
        </h3>
        <p className="text-sm text-text-secondary font-mono whitespace-pre-wrap break-words">
          {task.prompt}
        </p>
      </div>

      {/* Report card */}
      <div className="bg-bg-surface border border-border rounded-md p-4 md:col-span-2">
        <h3 className="text-sm font-semibold mb-3 text-text-primary">
          Report
        </h3>
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
          <StatCard label="Rounds Used" value={String(task.rounds_used)} sub={`of ${task.max_rounds}`} />
          <StatCard
            label="Lint"
            value={task.lint_status ?? "N/A"}
            color={
              task.lint_status === "pass"
                ? "text-status-success"
                : task.lint_status === "fail"
                  ? "text-status-failed"
                  : "text-text-muted"
            }
          />
          <StatCard
            label="Tests"
            value={task.test_status ?? "N/A"}
            color={
              task.test_status === "pass"
                ? "text-status-success"
                : task.test_status === "fail"
                  ? "text-status-failed"
                  : "text-text-muted"
            }
          />
          <StatCard
            label="Changes"
            value={`+${task.lines_added} / -${task.lines_removed}`}
            sub={task.files_changed ? `${task.files_changed.split(",").filter(Boolean).length} files` : undefined}
          />
        </div>
      </div>
    </div>
  );
}

function InfoRow({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="flex justify-between">
      <dt className="text-text-secondary">{label}</dt>
      <dd className={`text-text-primary ${mono ? "font-mono" : ""}`}>
        {value}
      </dd>
    </div>
  );
}

function StatCard({
  label,
  value,
  sub,
  color,
}: {
  label: string;
  value: string;
  sub?: string;
  color?: string;
}) {
  return (
    <div className="text-center">
      <div className={`text-lg font-bold ${color ?? "text-text-primary"}`}>
        {value}
      </div>
      <div className="text-xs text-text-secondary">{label}</div>
      {sub && <div className="text-xs text-text-muted">{sub}</div>}
    </div>
  );
}
