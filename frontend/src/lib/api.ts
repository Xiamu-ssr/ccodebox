import type {
  Task,
  CreateTaskRequest,
  CreateTaskResponse,
  TaskListResponse,
  SettingsResponse,
  ConfigItem,
  TestResult,
  AgentType,
  Project,
  CreateProjectRequest,
  ProjectListResponse,
  StageRun,
  RunStageRequest,
  TaskTypeListResponse,
  Template,
  TemplateListResponse,
  CreateTemplateRequest,
  UpdateTemplateRequest,
  AgentInfo,
} from "./types.generated";

const API_BASE = "/api";

async function fetchJSON<T>(url: string, init?: RequestInit): Promise<T> {
  const fullUrl = `${API_BASE}${url}`;
  console.log(`Fetching: ${fullUrl}`);
  const res = await fetch(fullUrl, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...init?.headers,
    },
  });

  if (!res.ok) {
    const text = await res.text();
    console.error(`Fetch failed for ${fullUrl}: ${res.status} ${res.statusText}. Response: ${text.substring(0, 100)}`);
    throw new Error(`API error: ${res.status} ${res.statusText}`);
  }

  const data = await res.json();
  return data as T;
}

// ── Tasks ──

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

export async function getTaskStages(taskId: string): Promise<StageRun[]> {
  return fetchJSON(`/tasks/${taskId}/stages`);
}

export async function cancelTask(id: string): Promise<void> {
  const res = await fetch(`${API_BASE}/tasks/${id}/cancel`, { method: "POST" });
  if (!res.ok) {
    throw new Error(`Failed to cancel task: ${res.status} ${res.statusText}`);
  }
}

// ── Stage Runs ──

export async function getStageRun(id: string): Promise<StageRun> {
  return fetchJSON(`/stage-runs/${id}`);
}

export async function stopStageRun(id: string): Promise<void> {
  const res = await fetch(`${API_BASE}/stage-runs/${id}/stop`, { method: "POST" });
  if (!res.ok) {
    throw new Error(`Failed to stop stage run: ${res.status} ${res.statusText}`);
  }
}

export async function runStage(
  req: RunStageRequest
): Promise<CreateTaskResponse> {
  return fetchJSON("/run", {
    method: "POST",
    body: JSON.stringify(req),
  });
}

// ── Projects ──

export async function createProject(
  req: CreateProjectRequest
): Promise<Project> {
  return fetchJSON("/projects", {
    method: "POST",
    body: JSON.stringify(req),
  });
}

export async function listProjects(): Promise<ProjectListResponse> {
  return fetchJSON("/projects");
}

export async function getProject(id: string): Promise<Project> {
  return fetchJSON(`/projects/${id}`);
}

export async function deleteProject(id: string): Promise<void> {
  await fetch(`${API_BASE}/projects/${id}`, { method: "DELETE" });
}

// ── Task Types / Templates ──

export async function getAgents(): Promise<AgentInfo[]> {
  return fetchJSON("/agents");
}

export async function listTaskTypes(): Promise<TaskTypeListResponse> {
  return fetchJSON("/task-types");
}

export async function listTemplates(): Promise<TemplateListResponse> {
  return fetchJSON("/templates");
}

export async function getTemplate(name: string): Promise<Template> {
  return fetchJSON(`/templates/${name}`);
}

export async function createTemplate(req: CreateTemplateRequest): Promise<Template> {
  return fetchJSON("/templates", {
    method: "POST",
    body: JSON.stringify(req),
  });
}

export async function updateTemplate(name: string, req: UpdateTemplateRequest): Promise<Template> {
  return fetchJSON(`/templates/${name}`, {
    method: "PUT",
    body: JSON.stringify(req),
  });
}

export async function deleteTemplate(name: string): Promise<void> {
  await fetch(`${API_BASE}/templates/${name}`, { method: "DELETE" });
}

// ── Settings ──

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
