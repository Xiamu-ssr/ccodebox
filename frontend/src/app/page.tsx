"use client";

import { useEffect, useState, useCallback } from "react";
import Link from "next/link";
import type { Task, TaskStatus } from "@/lib/types.generated";
import { listTasks } from "@/lib/api";
import TaskCard from "@/components/TaskCard";

const STATUS_FILTERS: { label: string; value: TaskStatus | "all" }[] = [
  { label: "All", value: "all" },
  { label: "Running", value: "running" },
  { label: "Success", value: "success" },
  { label: "Failed", value: "failed" },
  { label: "Pending", value: "pending" },
  { label: "Cancelled", value: "cancelled" },
];

export default function TaskListPage() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [total, setTotal] = useState(0);
  const [filter, setFilter] = useState<TaskStatus | "all">("all");
  const [loading, setLoading] = useState(true);

  const fetchTasks = useCallback(async () => {
    try {
      const res = await listTasks({
        status: filter === "all" ? undefined : filter,
        limit: 50,
      });
      setTasks(res.tasks);
      setTotal(res.total);
    } catch (err) {
      console.error("Failed to fetch tasks:", err);
    } finally {
      setLoading(false);
    }
  }, [filter]);

  useEffect(() => {
    setLoading(true);
    fetchTasks();
  }, [fetchTasks]);

  // Poll for updates when there are running tasks
  useEffect(() => {
    const hasRunning = tasks.some((t) => t.status === "running");
    if (!hasRunning) return;

    const interval = setInterval(fetchTasks, 3000);
    return () => clearInterval(interval);
  }, [tasks, fetchTasks]);

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-xl font-bold">Tasks</h1>
          <p className="text-sm text-text-secondary mt-1">
            {total} task{total !== 1 ? "s" : ""}
          </p>
        </div>
        <Link
          href="/tasks/new"
          className="bg-primary hover:bg-primary-hover text-white px-4 py-2 rounded-md text-sm font-medium transition-colors"
        >
          New Task
        </Link>
      </div>

      {/* Status filter */}
      <div className="flex gap-2 mb-4 overflow-x-auto pb-2">
        {STATUS_FILTERS.map((f) => (
          <button
            key={f.value}
            onClick={() => setFilter(f.value)}
            className={`px-3 py-1 rounded-full text-xs font-medium transition-colors whitespace-nowrap ${
              filter === f.value
                ? "bg-primary text-white"
                : "bg-bg-surface text-text-secondary hover:text-text-primary border border-border"
            }`}
          >
            {f.label}
          </button>
        ))}
      </div>

      {/* Task list */}
      {loading ? (
        <div className="flex items-center justify-center py-20">
          <div className="w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin" />
        </div>
      ) : tasks.length === 0 ? (
        <div className="text-center py-20 text-text-secondary">
          <p className="text-lg">No tasks yet</p>
          <p className="text-sm mt-2">
            Create your first task to get started.
          </p>
        </div>
      ) : (
        <div className="grid gap-3">
          {tasks.map((task) => (
            <TaskCard key={task.id} task={task} />
          ))}
        </div>
      )}
    </div>
  );
}
