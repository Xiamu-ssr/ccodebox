"use client";

import { useEffect, useRef, useState } from "react";
import { getTaskLogs } from "@/lib/api";

export default function LogViewer({
  taskId,
  isRunning,
}: {
  taskId: string;
  isRunning: boolean;
}) {
  const [logs, setLogs] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const containerRef = useRef<HTMLPreElement>(null);
  const autoScroll = useRef(true);

  useEffect(() => {
    let cancelled = false;

    async function fetchLogs() {
      try {
        const res = await getTaskLogs(taskId);
        if (!cancelled) {
          setLogs(res.logs);
          setError(null);
          setLoading(false);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Failed to load logs");
          setLoading(false);
        }
      }
    }

    fetchLogs();

    if (isRunning) {
      const interval = setInterval(fetchLogs, 3000);
      return () => {
        cancelled = true;
        clearInterval(interval);
      };
    }

    return () => {
      cancelled = true;
    };
  }, [taskId, isRunning]);

  useEffect(() => {
    if (autoScroll.current && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [logs]);

  function handleScroll() {
    if (!containerRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    autoScroll.current = scrollHeight - scrollTop - clientHeight < 50;
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-10">
        <div className="w-5 h-5 border-2 border-primary border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="text-center py-10 text-text-secondary text-sm">
        {error.includes("404") ? "No logs for this task." : `Error loading logs: ${error}`}
      </div>
    );
  }

  if (!logs) {
    return (
      <div className="text-center py-10 text-text-secondary text-sm">
        {isRunning ? "Waiting for logs..." : "No logs available."}
      </div>
    );
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-2">
        <span className="text-xs text-text-secondary">Agent Log</span>
        {isRunning && (
          <span className="text-xs text-status-running flex items-center gap-1">
            <span className="w-1.5 h-1.5 bg-status-running rounded-full animate-pulse" />
            Live
          </span>
        )}
      </div>
      <pre
        ref={containerRef}
        onScroll={handleScroll}
        className="bg-bg-base border border-border rounded-md p-4 text-xs font-mono text-text-primary overflow-auto max-h-[600px] whitespace-pre-wrap break-words"
      >
        {logs}
      </pre>
    </div>
  );
}
