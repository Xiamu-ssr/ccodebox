import TaskDetailClient from "./client";

export default async function TaskDetailPage(props: {
  params: Promise<{ id: string, taskId: string }>;
}) {
  const { id, taskId } = await props.params;
  return <TaskDetailClient projectId={id} taskId={taskId} />;
}