import Client from "./client";

export default async function ProjectDetailPage(props: {
  params: Promise<{ id: string }>;
}) {
  return <Client params={props.params} />;
}