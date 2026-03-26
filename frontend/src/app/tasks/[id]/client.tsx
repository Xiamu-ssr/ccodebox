"use client";

import TaskDetail from "@/components/TaskDetail";

export default function TaskDetailClient({ taskId }: { taskId: string }) {
  return <TaskDetail taskId={taskId} />;
}
