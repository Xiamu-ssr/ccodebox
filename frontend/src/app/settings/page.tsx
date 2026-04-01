"use client";

import { useEffect, useState, useCallback } from "react";
import type { AgentInfo, AgentType, ConfigItem } from "@/lib/types.generated";
import { getSettings, updateSettings, testAgent, testTool } from "@/lib/api";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/ui/badge";
import { IconRobot, IconSettings, IconGitBranch, IconSearch, IconEye, IconEyeOff, IconCheck, IconX, IconLoader2 } from "@tabler/icons-react";

// Config keys per agent type
const AGENT_CONFIG: Record<string, { keyPrefix: string }> = {
  "claude-code": { keyPrefix: "agent.claude-code" },
  codex: { keyPrefix: "agent.codex" },
};

export default function SettingsPage() {
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [configMap, setConfigMap] = useState<Record<string, string>>({});
  const [savedConfig, setSavedConfig] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

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

  useEffect(() => {
    fetchSettings();
  }, [fetchSettings]);

  const dirty = JSON.stringify(configMap) !== JSON.stringify(savedConfig);

  const updateField = (key: string, value: string) => {
    setConfigMap((prev) => ({ ...prev, [key]: value }));
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      const items: ConfigItem[] = Object.entries(configMap).map(([key, value]) => ({
        key,
        value,
        encrypted: key.includes("api_key") || key.includes("token"),
      }));
      await updateSettings(items);
      await fetchSettings();
    } catch (err) {
      console.error("Failed to save settings:", err);
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="space-y-6 max-w-4xl">
        <Skeleton className="h-10 w-32" />
        <Card>
          <CardHeader>
            <Skeleton className="h-6 w-1/4 mb-2" />
            <Skeleton className="h-4 w-1/2" />
          </CardHeader>
          <CardContent className="space-y-4">
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-full" />
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-8 max-w-4xl pb-10">
      <div className="flex items-center justify-between sticky top-14 bg-background/95 backdrop-blur z-10 py-4 -my-4 border-b border-border/50">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Settings</h1>
          <p className="text-muted-foreground mt-1">
            Manage API keys and platform configuration
          </p>
        </div>
        <Button
          onClick={handleSave}
          disabled={!dirty || saving}
        >
          {saving && <IconLoader2 className="w-4 h-4 mr-2 animate-spin" />}
          {saving ? "Saving..." : "Save Changes"}
        </Button>
      </div>

      <div className="grid gap-6 mt-6">
        {/* Agent Configuration */}
        <section className="space-y-4">
          <div className="flex items-center gap-2 mb-2">
            <IconRobot className="h-5 w-5 text-primary" />
            <h2 className="text-xl font-semibold tracking-tight">Agent Configuration</h2>
          </div>
          <div className="grid gap-4 md:grid-cols-2">
            {agents.map((agent) => {
              const cfg = AGENT_CONFIG[agent.type];
              if (!cfg) return null;

              return (
                <Card key={agent.type} className="flex flex-col">
                  <CardHeader className="pb-3 flex-none">
                    <div className="flex items-start justify-between">
                      <div>
                        <CardTitle className="text-lg flex items-center gap-2">
                          {agent.name}
                          {agent.installed ? (
                            <Badge variant="outline" className="bg-green-500/10 text-green-600 dark:text-green-400 border-green-500/20">Installed</Badge>
                          ) : (
                            <Badge variant="outline" className="text-muted-foreground">Not Installed</Badge>
                          )}
                        </CardTitle>
                      </div>
                    </div>
                  </CardHeader>
                  <CardContent className="space-y-4 flex-1">
                    <div className="space-y-3">
                      <div className="space-y-1.5">
                        <ConfigField
                          label="Default Model"
                          configKey={`${cfg.keyPrefix}.default_model`}
                          value={configMap[`${cfg.keyPrefix}.default_model`] ?? ""}
                          onChange={updateField}
                          placeholder="e.g. sonnet, opus, o3"
                        />
                        <p className="text-xs text-muted-foreground">
                          Model to use when not specified per-task
                        </p>
                      </div>
                    </div>
                  </CardContent>
                </Card>
              );
            })}
          </div>
        </section>

        {/* Tools Configuration */}
        <section className="space-y-4">
          <div className="flex items-center gap-2 mb-2">
            <IconSettings className="h-5 w-5 text-primary" />
            <h2 className="text-xl font-semibold tracking-tight">Tools</h2>
          </div>
          <Card>
            <CardHeader className="pb-3">
              <div className="flex items-start justify-between">
                <div>
                  <CardTitle className="text-lg flex items-center gap-2">
                    <IconSearch className="h-4 w-4 text-muted-foreground" />
                    Tavily Search
                  </CardTitle>
                  <CardDescription>Web search capability for agents</CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-1.5">
                <ConfigField
                  label="API Key"
                  configKey="tool.tavily.api_key"
                  value={configMap["tool.tavily.api_key"] ?? ""}
                  onChange={updateField}
                  sensitive
                />
                <p className="text-xs text-muted-foreground">
                  Optional — used by platform steward for web search
                </p>
              </div>
            </CardContent>
          </Card>
        </section>

        {/* Git Configuration */}
        <section className="space-y-4">
          <div className="flex items-center gap-2 mb-2">
            <IconGitBranch className="h-5 w-5 text-primary" />
            <h2 className="text-xl font-semibold tracking-tight">Git</h2>
          </div>
          <Card>
            <CardContent className="pt-6">
              <div className="space-y-1.5">
                <ConfigField
                  label="GitHub Personal Access Token"
                  configKey="git.github_token"
                  value={configMap["git.github_token"] ?? ""}
                  onChange={updateField}
                  sensitive
                />
                <p className="text-xs text-muted-foreground">
                  Required for cloning private repos and pushing branches
                </p>
              </div>
            </CardContent>
          </Card>
        </section>
      </div>
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
    <div className="space-y-1.5">
      <Label htmlFor={configKey}>{label}</Label>
      <div className="flex relative">
        <Input
          id={configKey}
          type={sensitive && !visible ? "password" : "text"}
          value={value}
          onChange={(e) => onChange(configKey, e.target.value)}
          placeholder={placeholder ?? (sensitive ? "Enter value..." : "")}
          className={sensitive ? "pr-10" : ""}
        />
        {sensitive && (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={() => setVisible(!visible)}
            className="absolute right-0 top-0 h-full text-muted-foreground hover:text-foreground px-3"
            tabIndex={-1}
          >
            {visible ? <IconEyeOff className="h-4 w-4" /> : <IconEye className="h-4 w-4" />}
          </Button>
        )}
      </div>
      {isMasked && (
        <p className="text-xs text-muted-foreground">
          Value is saved and masked. Enter a new value to replace it.
        </p>
      )}
    </div>
  );
}
