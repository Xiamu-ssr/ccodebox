"use client";

import Link from "next/link";
import type { Task } from "@/lib/types.generated";
import StatusBadge from "./StatusBadge";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";

function formatTime(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString();
}

export default function TaskCard({
  task,
  projectName,
}: {
  task: Task;
  projectName?: string;
}) {
  // Duration calculation
  let durationStr = "";
  if (task.started_at) {
    const end = task.finished_at ? new Date(task.finished_at).getTime() : Date.now();
    const start = new Date(task.started_at).getTime();
    const diffSeconds = Math.floor((end - start) / 1000);
    if (diffSeconds < 60) {
      durationStr = `${diffSeconds}s`;
    } else {
      durationStr = `${Math.floor(diffSeconds / 60)}m ${diffSeconds % 60}s`;
    }
  }

  return (
    <Link href={task.project_id ? `/projects/${task.project_id}/tasks/${task.id}` : `/tasks/${task.id}`} className="block">
      <Card className="hover:border-primary/50 transition-colors">
        <CardHeader className="py-3">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              <CardTitle className="text-base truncate group-hover:text-primary transition-colors">
                {task.title}
              </CardTitle>
              <CardDescription className="text-xs mt-1 line-clamp-2">
                {task.prompt}
              </CardDescription>
            </div>
            <StatusBadge status={task.status} />
          </div>
        </CardHeader>
        <CardContent className="py-3 pt-0 flex items-center gap-3 text-xs text-muted-foreground">
          {projectName && (
            <>
              <span className="font-medium">{projectName}</span>
              <span>|</span>
            </>
          )}
          <span className="font-mono bg-muted px-1.5 py-0.5 rounded">{task.task_type}</span>
          {task.current_stage && (
            <>
              <span>|</span>
              <span className="text-primary font-mono">
                {task.current_stage}
              </span>
            </>
          )}
          <span className="flex-1" />
          {durationStr && <span>{durationStr}</span>}
        </CardContent>
      </Card>
    </Link>
  );
}
