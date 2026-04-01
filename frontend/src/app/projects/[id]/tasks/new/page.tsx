import Client from "./client";

export default async function NewTaskPage(props: {
  params: Promise<{ id: string }>;
}) {
  return <Client params={props.params} />;
}