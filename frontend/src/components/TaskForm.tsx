"use client";

import { useState, useEffect } from "react";
import { useRouter } from "next/navigation";
import type { SettingsResponse, AgentType } from "@/lib/types.generated";
import { createTask, getSettings } from "@/lib/api";

export default function TaskForm() {
  const router = useRouter();
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [title, setTitle] = useState("");
  const [prompt, setPrompt] = useState("");
  const [repoUrl, setRepoUrl] = useState("");
  const [branch, setBranch] = useState("");
  const [agentType, setAgentType] = useState<AgentType>("claude_code");
  const [model, setModel] = useState("");
  useEffect(() => {
    getSettings().then((s) => {
      setSettings(s);
      setModel(s.default_model);
      if (s.agents.length > 0) {
        setAgentType(s.agents[0].type);
      }
    });
  }, []);

  const selectedAgent = settings?.agents.find((a) => a.type === agentType);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!title.trim() || !prompt.trim()) return;

    setSubmitting(true);
    setError(null);

    try {
      const res = await createTask({
        title: title.trim(),
        prompt: prompt.trim(),
        repo_url: repoUrl.trim() || null,
        branch: branch.trim() || null,
        agent_type: agentType,
        model: model || null,
      });
      router.push(`/tasks/${res.id}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create task");
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="max-w-2xl space-y-5">
      {error && (
        <div className="bg-status-failed/10 border border-status-failed/30 text-status-failed rounded-md p-3 text-sm">
          {error}
        </div>
      )}

      {/* Title */}
      <div>
        <label className="block text-sm font-medium text-text-primary mb-1.5">
          Title <span className="text-status-failed">*</span>
        </label>
        <input
          type="text"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="e.g., Implement calculator module"
          required
          className="w-full bg-bg-base border border-border rounded-md px-3 py-2 text-sm text-text-primary placeholder-text-muted focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary"
        />
      </div>

      {/* Prompt */}
      <div>
        <label className="block text-sm font-medium text-text-primary mb-1.5">
          Prompt <span className="text-status-failed">*</span>
        </label>
        <textarea
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder="Describe the task in detail..."
          required
          rows={6}
          className="w-full bg-bg-base border border-border rounded-md px-3 py-2 text-sm text-text-primary placeholder-text-muted focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary font-mono resize-y"
        />
      </div>

      {/* Repo URL */}
      <div>
        <label className="block text-sm font-medium text-text-primary mb-1.5">
          Repository URL{" "}
          <span className="text-text-muted font-normal">(optional)</span>
        </label>
        <input
          type="url"
          value={repoUrl}
          onChange={(e) => setRepoUrl(e.target.value)}
          placeholder="https://github.com/user/repo"
          className="w-full bg-bg-base border border-border rounded-md px-3 py-2 text-sm text-text-primary placeholder-text-muted focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary"
        />
      </div>

      {/* Branch */}
      <div>
        <label className="block text-sm font-medium text-text-primary mb-1.5">
          Branch{" "}
          <span className="text-text-muted font-normal">(optional)</span>
        </label>
        <input
          type="text"
          value={branch}
          onChange={(e) => setBranch(e.target.value)}
          placeholder="feat/my-feature"
          className="w-full bg-bg-base border border-border rounded-md px-3 py-2 text-sm text-text-primary placeholder-text-muted focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary"
        />
      </div>

      {/* Agent + Model row */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium text-text-primary mb-1.5">
            Agent
          </label>
          <select
            value={agentType}
            onChange={(e) => {
              const val = e.target.value as AgentType;
              setAgentType(val);
              // Reset model to default when switching agents
              if (settings) {
                setModel(settings.default_model);
              }
            }}
            className="w-full bg-bg-base border border-border rounded-md px-3 py-2 text-sm text-text-primary focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary"
          >
            {settings?.agents.map((a) => (
              <option key={a.type} value={a.type}>
                {a.name}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label className="block text-sm font-medium text-text-primary mb-1.5">
            Model
          </label>
          <input
            type="text"
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder={settings?.default_model ?? "Enter model name"}
            className="w-full bg-bg-base border border-border rounded-md px-3 py-2 text-sm text-text-primary placeholder-text-muted focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary"
          />
          {selectedAgent && (
            <p className="text-xs text-text-muted mt-1">
              {selectedAgent.name} agent
            </p>
          )}
        </div>
      </div>

      {/* Submit */}
      <button
        type="submit"
        disabled={submitting || !title.trim() || !prompt.trim()}
        className="w-full bg-primary hover:bg-primary-hover disabled:opacity-50 disabled:cursor-not-allowed text-white font-medium py-2.5 rounded-md transition-colors text-sm"
      >
        {submitting ? (
          <span className="flex items-center justify-center gap-2">
            <span className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
            Creating...
          </span>
        ) : (
          "Create Task"
        )}
      </button>
    </form>
  );
}
