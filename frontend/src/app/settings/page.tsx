"use client";

import { useEffect, useState, useCallback } from "react";
import type {
  AgentInfo,
  AgentType,
  ConfigItem,
} from "@/lib/types.generated";
import {
  getSettings,
  updateSettings,
  testAgent,
  testTool,
  getImageStatus,
  buildImages,
} from "@/lib/api";

// Config keys per agent type
const AGENT_CONFIG: Record<
  string,
  { keyPrefix: string; hasBaseUrl: boolean }
> = {
  claude_code: { keyPrefix: "agent.claude-code", hasBaseUrl: true },
  codex: { keyPrefix: "agent.codex", hasBaseUrl: true },
};

interface ImageStatus {
  name: string;
  ready: boolean;
}

export default function SettingsPage() {
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [configMap, setConfigMap] = useState<Record<string, string>>({});
  const [savedConfig, setSavedConfig] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testResults, setTestResults] = useState<
    Record<string, { success: boolean; message: string } | null>
  >({});
  const [testing, setTesting] = useState<Record<string, boolean>>({});
  const [images, setImages] = useState<ImageStatus[]>([]);
  const [building, setBuilding] = useState(false);

  const fetchSettings = useCallback(async () => {
    try {
      const res = await getSettings();
      setAgents(res.agents);
      const map: Record<string, string> = {};
      for (const item of res.config) {
        map[item.key] = item.value;
      }
      setConfigMap(map);
      setSavedConfig(map);
    } catch (err) {
      console.error("Failed to fetch settings:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  const fetchImages = useCallback(async () => {
    try {
      const statuses = await getImageStatus();
      setImages(statuses);
    } catch (err) {
      console.error("Failed to fetch image status:", err);
    }
  }, []);

  useEffect(() => {
    fetchSettings();
    fetchImages();
  }, [fetchSettings, fetchImages]);

  const dirty = JSON.stringify(configMap) !== JSON.stringify(savedConfig);

  const updateField = (key: string, value: string) => {
    setConfigMap((prev) => ({ ...prev, [key]: value }));
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      const items: ConfigItem[] = Object.entries(configMap).map(
        ([key, value]) => ({
          key,
          value,
          encrypted: key.includes("api_key") || key.includes("token"),
        })
      );
      await updateSettings(items);
      await fetchSettings();
    } catch (err) {
      console.error("Failed to save settings:", err);
    } finally {
      setSaving(false);
    }
  };

  const handleTestAgent = async (agentType: AgentType) => {
    const key = `agent-${agentType}`;
    setTesting((prev) => ({ ...prev, [key]: true }));
    setTestResults((prev) => ({ ...prev, [key]: null }));
    try {
      const result = await testAgent(agentType);
      setTestResults((prev) => ({ ...prev, [key]: result }));
    } catch (err) {
      setTestResults((prev) => ({
        ...prev,
        [key]: { success: false, message: String(err) },
      }));
    } finally {
      setTesting((prev) => ({ ...prev, [key]: false }));
    }
  };

  const handleTestTool = async (tool: string) => {
    const key = `tool-${tool}`;
    setTesting((prev) => ({ ...prev, [key]: true }));
    setTestResults((prev) => ({ ...prev, [key]: null }));
    try {
      const result = await testTool(tool);
      setTestResults((prev) => ({ ...prev, [key]: result }));
    } catch (err) {
      setTestResults((prev) => ({
        ...prev,
        [key]: { success: false, message: String(err) },
      }));
    } finally {
      setTesting((prev) => ({ ...prev, [key]: false }));
    }
  };

  const handleBuildImages = async () => {
    setBuilding(true);
    try {
      await buildImages();
      // Poll for completion
      const poll = setInterval(async () => {
        const statuses = await getImageStatus();
        setImages(statuses);
        if (statuses.every((s) => s.ready)) {
          clearInterval(poll);
          setBuilding(false);
        }
      }, 5000);
    } catch (err) {
      console.error("Failed to start image build:", err);
      setBuilding(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  const allImagesReady = images.length > 0 && images.every((i) => i.ready);

  return (
    <div className="max-w-3xl">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-xl font-bold">Settings</h1>
          <p className="text-sm text-text-secondary mt-1">
            Manage API keys and platform configuration
          </p>
        </div>
        <button
          onClick={handleSave}
          disabled={!dirty || saving}
          className="bg-primary hover:bg-primary-hover disabled:opacity-50 disabled:cursor-not-allowed text-white px-4 py-2 rounded-md text-sm font-medium transition-colors"
        >
          {saving ? "Saving..." : "Save Changes"}
        </button>
      </div>

      {/* Docker Images */}
      <section className="mb-8">
        <h2 className="text-lg font-semibold mb-4">Agent Images</h2>
        <div className="bg-bg-surface border border-border rounded-lg p-4">
          <div className="space-y-2 mb-3">
            {images.map((img) => (
              <div
                key={img.name}
                className="flex items-center justify-between text-sm"
              >
                <span className="font-mono text-text-primary">{img.name}</span>
                <span
                  className={
                    img.ready ? "text-green-400" : "text-red-400"
                  }
                >
                  {img.ready ? "Ready" : "Not Built"}
                </span>
              </div>
            ))}
            {images.length === 0 && (
              <p className="text-sm text-text-secondary">
                Could not check image status. Is Docker running?
              </p>
            )}
          </div>
          {!allImagesReady && (
            <button
              onClick={handleBuildImages}
              disabled={building}
              className="text-xs bg-primary hover:bg-primary-hover disabled:opacity-50 text-white px-3 py-1.5 rounded transition-colors"
            >
              {building ? "Building..." : "Build All Images"}
            </button>
          )}
        </div>
      </section>

      {/* Agent Configuration */}
      <section className="mb-8">
        <h2 className="text-lg font-semibold mb-4">Agent Configuration</h2>
        <div className="grid gap-4">
          {agents.map((agent) => {
            const cfg = AGENT_CONFIG[agent.type];
            if (!cfg) return null;
            const testKey = `agent-${agent.type}`;
            const result = testResults[testKey];
            const isTesting = testing[testKey];

            return (
              <div
                key={agent.type}
                className="bg-bg-surface border border-border rounded-lg p-4"
              >
                <div className="flex items-center justify-between mb-3">
                  <div>
                    <h3 className="font-medium">{agent.name}</h3>
                    <p className="text-xs text-text-secondary">
                      {agent.image}
                    </p>
                  </div>
                  <button
                    onClick={() => handleTestAgent(agent.type)}
                    disabled={isTesting}
                    className="text-xs border border-border px-3 py-1 rounded hover:bg-bg-surface-hover transition-colors disabled:opacity-50"
                  >
                    {isTesting ? "Testing..." : "Test Connection"}
                  </button>
                </div>

                {result && (
                  <div
                    className={`text-xs px-3 py-2 rounded mb-3 ${
                      result.success
                        ? "bg-green-500/10 text-green-400"
                        : "bg-red-500/10 text-red-400"
                    }`}
                  >
                    {result.message}
                  </div>
                )}

                <div className="grid gap-3">
                  <ConfigField
                    label="API Key"
                    configKey={`${cfg.keyPrefix}.api_key`}
                    value={configMap[`${cfg.keyPrefix}.api_key`] ?? ""}
                    onChange={updateField}
                    sensitive
                  />
                  {cfg.hasBaseUrl && (
                    <ConfigField
                      label="API Base URL"
                      configKey={`${cfg.keyPrefix}.api_base_url`}
                      value={
                        configMap[`${cfg.keyPrefix}.api_base_url`] ?? ""
                      }
                      onChange={updateField}
                      placeholder={
                        agent.type === "codex"
                          ? "https://api.openai.com"
                          : "https://api.anthropic.com"
                      }
                    />
                  )}
                  <ConfigField
                    label="Default Model"
                    configKey={`${cfg.keyPrefix}.default_model`}
                    value={
                      configMap[`${cfg.keyPrefix}.default_model`] ?? ""
                    }
                    onChange={updateField}
                    placeholder="Enter model name"
                  />
                </div>
              </div>
            );
          })}
        </div>
      </section>

      {/* Tool Configuration */}
      <section className="mb-8">
        <h2 className="text-lg font-semibold mb-4">Tools</h2>
        <div className="bg-bg-surface border border-border rounded-lg p-4">
          <div className="flex items-center justify-between mb-3">
            <div>
              <h3 className="font-medium">Tavily Search</h3>
              <p className="text-xs text-text-secondary">
                Web search for agents
              </p>
            </div>
            <button
              onClick={() => handleTestTool("tavily")}
              disabled={testing["tool-tavily"]}
              className="text-xs border border-border px-3 py-1 rounded hover:bg-bg-surface-hover transition-colors disabled:opacity-50"
            >
              {testing["tool-tavily"] ? "Testing..." : "Test"}
            </button>
          </div>

          {testResults["tool-tavily"] && (
            <div
              className={`text-xs px-3 py-2 rounded mb-3 ${
                testResults["tool-tavily"]!.success
                  ? "bg-green-500/10 text-green-400"
                  : "bg-red-500/10 text-red-400"
              }`}
            >
              {testResults["tool-tavily"]!.message}
            </div>
          )}

          <ConfigField
            label="API Key"
            configKey="tool.tavily.api_key"
            value={configMap["tool.tavily.api_key"] ?? ""}
            onChange={updateField}
            sensitive
          />
        </div>
      </section>

      {/* Git Configuration */}
      <section className="mb-8">
        <h2 className="text-lg font-semibold mb-4">Git</h2>
        <div className="bg-bg-surface border border-border rounded-lg p-4">
          <h3 className="font-medium mb-3">GitHub</h3>
          <ConfigField
            label="Personal Access Token"
            configKey="git.github_token"
            value={configMap["git.github_token"] ?? ""}
            onChange={updateField}
            sensitive
          />
        </div>
      </section>
    </div>
  );
}

function ConfigField({
  label,
  configKey,
  value,
  onChange,
  sensitive,
  placeholder,
}: {
  label: string;
  configKey: string;
  value: string;
  onChange: (key: string, value: string) => void;
  sensitive?: boolean;
  placeholder?: string;
}) {
  const [visible, setVisible] = useState(false);
  const isMasked = sensitive && value.includes("***");

  return (
    <div>
      <label className="block text-xs text-text-secondary mb-1">
        {label}
      </label>
      <div className="flex gap-2">
        <input
          type={sensitive && !visible ? "password" : "text"}
          value={value}
          onChange={(e) => onChange(configKey, e.target.value)}
          placeholder={placeholder ?? (sensitive ? "Enter value..." : "")}
          className="flex-1 bg-bg-primary border border-border rounded px-3 py-1.5 text-sm focus:outline-none focus:border-primary"
        />
        {sensitive && (
          <button
            type="button"
            onClick={() => setVisible(!visible)}
            className="text-xs text-text-secondary hover:text-text-primary px-2"
          >
            {visible ? "Hide" : "Show"}
          </button>
        )}
      </div>
      {isMasked && (
        <p className="text-xs text-text-secondary mt-1">
          Value is saved. Enter a new value to replace it.
        </p>
      )}
    </div>
  );
}
