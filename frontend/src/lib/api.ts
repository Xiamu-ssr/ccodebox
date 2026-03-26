import type {
  Task,
  CreateTaskRequest,
  CreateTaskResponse,
  TaskListResponse,
  TaskLogsResponse,
  SettingsResponse,
  ConfigItem,
  TestResult,
  AgentType,
} from "./types.generated";

const API_BASE = "/api";

async function fetchJSON<T>(url: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${url}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...init?.headers,
    },
  });

  if (!res.ok) {
    const body = await res.text();
    throw new Error(`API error ${res.status}: ${body}`);
  }

  return res.json();
}

export async function createTask(
  req: CreateTaskRequest
): Promise<CreateTaskResponse> {
  return fetchJSON("/tasks", {
    method: "POST",
    body: JSON.stringify(req),
  });
}

export async function listTasks(params?: {
  status?: string;
  limit?: number;
  offset?: number;
}): Promise<TaskListResponse> {
  const search = new URLSearchParams();
  if (params?.status) search.set("status", params.status);
  if (params?.limit) search.set("limit", String(params.limit));
  if (params?.offset) search.set("offset", String(params.offset));
  const qs = search.toString();
  return fetchJSON(`/tasks${qs ? `?${qs}` : ""}`);
}

export async function getTask(id: string): Promise<Task> {
  return fetchJSON(`/tasks/${id}`);
}

export async function getTaskLogs(id: string): Promise<TaskLogsResponse> {
  return fetchJSON(`/tasks/${id}/logs`);
}

export async function cancelTask(id: string): Promise<void> {
  await fetch(`${API_BASE}/tasks/${id}/cancel`, { method: "POST" });
}

export async function getSettings(): Promise<SettingsResponse> {
  return fetchJSON("/settings");
}

export async function updateSettings(
  config: ConfigItem[]
): Promise<void> {
  await fetchJSON("/settings", {
    method: "PUT",
    body: JSON.stringify({ config }),
  });
}

export async function testAgent(agentType: AgentType): Promise<TestResult> {
  return fetchJSON("/settings/test-agent", {
    method: "POST",
    body: JSON.stringify({ agent_type: agentType }),
  });
}

export async function testTool(tool: string): Promise<TestResult> {
  return fetchJSON("/settings/test-tool", {
    method: "POST",
    body: JSON.stringify({ tool }),
  });
}

export async function getImageStatus(): Promise<
  { name: string; ready: boolean }[]
> {
  return fetchJSON("/settings/images");
}

export async function buildImages(): Promise<void> {
  await fetch(`${API_BASE}/settings/images/build`, { method: "POST" });
}
