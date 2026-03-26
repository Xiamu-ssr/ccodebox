import TaskDetailClient from "./client";

export async function generateStaticParams(): Promise<{ id: string }[]> {
  return [{ id: "placeholder" }];
}

export default async function TaskDetailPage(props: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await props.params;
  return <TaskDetailClient taskId={id} />;
}
