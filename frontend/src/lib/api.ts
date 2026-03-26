import type { Task } from "./generated/Task";
import type { CreateTaskRequest } from "./generated/CreateTaskRequest";
import type { CreateTaskResponse } from "./generated/CreateTaskResponse";
import type { TaskListResponse } from "./generated/TaskListResponse";
import type { TaskLogsResponse } from "./generated/TaskLogsResponse";
import type { SettingsResponse } from "./generated/SettingsResponse";

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
