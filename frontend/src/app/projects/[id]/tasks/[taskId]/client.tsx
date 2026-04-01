"use client";

import TaskDetail from "@/components/TaskDetail";

export default function TaskDetailClient({ projectId, taskId }: { projectId: string, taskId: string }) {
  return <TaskDetail projectId={projectId} taskId={taskId} />;
}