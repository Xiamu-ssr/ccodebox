"use client";

import { useState, useEffect, useCallback } from "react";
import type { Project, AgentType, Task, StageRun, RunStageRequest, AgentInfo } from "@/lib/types.generated";
import { listProjects, runStage, getTask, getTaskStages, getAgents } from "@/lib/api";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ScrollArea } from "@/components/ui/scroll-area";
import DiffViewer from "@/components/DiffViewer";
import AgentLogViewer from "@/components/AgentLogViewer";
import PromptViewer from "@/components/PromptViewer";
import StatusBadge from "@/components/StatusBadge";
import { IconPlayerPlay, IconLoader2, IconTerminal2, IconFileCode, IconInfoCircle, IconAlertCircle } from "@tabler/icons-react";

export default function PlaygroundPage() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Form State
  const [projectId, setProjectId] = useState("");
  const [agentType, setAgentType] = useState<AgentType>("claude-code");
  const [model, setModel] = useState("");
  const [prompt, setPrompt] = useState("");

  // Result State
  const [taskId, setTaskId] = useState<string | null>(null);
  const [task, setTask] = useState<Task | null>(null);
  const [stageRun, setStageRun] = useState<StageRun | null>(null);

  useEffect(() => {
    Promise.all([
      listProjects(),
      getAgents()
    ])
      .then(([projRes, agentsRes]) => {
        setProjects(projRes.projects);
        setAgents(agentsRes);
        if (projRes.projects.length > 0) {
          setProjectId(projRes.projects[0].id);
        }
        
        // Find first installed agent to set as default
        const firstInstalled = agentsRes.find(a => a.installed);
        if (firstInstalled) {
          setAgentType(firstInstalled.type);
        }
      })
      .catch((err) => console.error("Failed to load playground data:", err))
      .finally(() => setLoading(false));
  }, []);

  const pollResult = useCallback(async (id: string) => {
    try {
      const [t, s] = await Promise.all([
        getTask(id),
        getTaskStages(id),
      ]);
      setTask(t);
      if (s.length > 0) {
        setStageRun(s[0]);
      }
      
      if (t.status === "running" || t.status === "pending") {
        setTimeout(() => pollResult(id), 3000);
      } else {
        setRunning(false);
      }
    } catch (err) {
      console.error("Failed to poll result:", err);
      setRunning(false);
    }
  }, []);

  async function handleRun(e: React.FormEvent) {
    e.preventDefault();
    if (!projectId || !prompt.trim()) return;

    setRunning(true);
    setError(null);
    setTaskId(null);
    setTask(null);
    setStageRun(null);

    try {
      const req: RunStageRequest = {
        project_id: projectId,
        agent_type: agentType,
        prompt: prompt.trim(),
        model: model.trim() || null,
      };
      const res = await runStage(req);
      setTaskId(res.id);
      pollResult(res.id);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to run stage");
      setRunning(false);
    }
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Playground</h1>
        <p className="text-muted-foreground mt-1">
          Test individual agents without full orchestration
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
        {/* Form Panel */}
        <Card className="lg:col-span-4 h-fit">
          <CardHeader>
            <CardTitle className="text-lg">Run Configuration</CardTitle>
          </CardHeader>
          <CardContent>
            <form onSubmit={handleRun} className="space-y-4">
              {error && (
                <div className="bg-destructive/10 border border-destructive/30 text-destructive rounded-md p-3 text-sm">
                  {error}
                </div>
              )}

              <div className="space-y-2">
                <Label>Project</Label>
                <Select value={projectId} onValueChange={(v) => setProjectId(v || "")} disabled={running || loading}>
                  <SelectTrigger>
                    <SelectValue placeholder="Select a project" />
                  </SelectTrigger>
                  <SelectContent>
                    {projects.map((p) => (
                      <SelectItem key={p.id} value={p.id}>{p.name}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-2">
                <Label>Agent</Label>
                <Select value={agentType} onValueChange={(v) => setAgentType((v || "claude-code") as AgentType)} disabled={running}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {agents
                      .filter(a => a.installed)
                      .map(a => (
                        <SelectItem key={a.type} value={a.type}>{a.name}</SelectItem>
                      ))
                    }
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-2">
                <Label>Model <span className="text-muted-foreground font-normal text-xs">(Optional)</span></Label>
                <Input
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                  placeholder="Leave empty for default"
                  disabled={running}
                />
              </div>

              <div className="space-y-2">
                <Label>Prompt <span className="text-destructive">*</span></Label>
                <Textarea
                  value={prompt}
                  onChange={(e) => setPrompt(e.target.value)}
                  placeholder="What do you want the agent to do?"
                  rows={8}
                  required
                  disabled={running}
                  className="font-mono text-sm resize-y"
                />
              </div>

              <Button
                type="submit"
                disabled={running || !projectId || !prompt.trim() || loading}
                className="w-full mt-2"
              >
                {running ? (
                  <>
                    <IconLoader2 className="mr-2 h-4 w-4 animate-spin" />
                    Running...
                  </>
                ) : (
                  <>
                    <IconPlayerPlay className="mr-2 h-4 w-4" />
                    Execute Stage
                  </>
                )}
              </Button>
            </form>
          </CardContent>
        </Card>

        {/* Results Panel */}
        <div className="lg:col-span-8">
          {!taskId ? (
            <div className="h-full min-h-[400px] border rounded-lg border-dashed flex flex-col items-center justify-center text-muted-foreground p-8">
              <IconTerminal2 className="h-12 w-12 opacity-20 mb-4" />
              <p>Configure and run a stage to see results here.</p>
            </div>
          ) : (
            <Card className="h-full flex flex-col">
              <CardHeader className="pb-3 border-b bg-muted/10 flex-none">
                <div className="flex items-center justify-between">
                  <CardTitle className="text-lg flex items-center gap-3">
                    Result
                    {task && <StatusBadge status={task.status} />}
                  </CardTitle>
                  {stageRun && (
                    <div className="text-sm text-muted-foreground flex gap-4">
                      {stageRun.duration_seconds != null && (
                        <span>Duration: {stageRun.duration_seconds}s</span>
                      )}
                      {stageRun.agent_exit_code != null && (
                        <span className={stageRun.agent_exit_code === 0 ? "text-green-500" : "text-destructive"}>
                          Exit Code: {stageRun.agent_exit_code}
                        </span>
                      )}
                    </div>
                  )}
                </div>
              </CardHeader>
              <CardContent className="p-0 flex-1 flex flex-col overflow-hidden min-h-[500px]">
                <Tabs defaultValue="log" className="flex flex-col h-full">
                  <div className="px-4 pt-3 border-b bg-muted/10">
                    <TabsList className="mb-3">
                      <TabsTrigger value="log" className="text-xs h-7">
                        <IconTerminal2 className="h-3 w-3 mr-1.5" /> Agent Log
                      </TabsTrigger>
                      <TabsTrigger value="diff" className="text-xs h-7" disabled={!stageRun?.diff_patch}>
                        <IconFileCode className="h-3 w-3 mr-1.5" /> Diff
                      </TabsTrigger>
                      <TabsTrigger value="error" className="text-xs h-7 text-destructive data-[state=active]:text-destructive" disabled={!stageRun?.error_report}>
                        <IconAlertCircle className="h-3 w-3 mr-1.5" /> Error
                      </TabsTrigger>
                      <TabsTrigger value="info" className="text-xs h-7">
                        <IconInfoCircle className="h-3 w-3 mr-1.5" /> Info
                      </TabsTrigger>
                    </TabsList>
                  </div>

                  <TabsContent value="log" className="flex-1 p-0 m-0 overflow-hidden bg-zinc-950 text-zinc-50 relative">
                    <AgentLogViewer log={stageRun?.agent_log || null} status={task?.status || "pending"} />
                  </TabsContent>

                  <TabsContent value="diff" className="flex-1 p-0 m-0 overflow-hidden">
                    <ScrollArea className="h-full w-full">
                      {stageRun?.diff_patch && <DiffViewer diff={stageRun.diff_patch} />}
                    </ScrollArea>
                  </TabsContent>

                  <TabsContent value="error" className="flex-1 p-0 m-0 overflow-hidden bg-destructive/5">
                    <ScrollArea className="h-full w-full p-4">
                      {stageRun?.error_report && (
                        <pre className="text-sm font-mono whitespace-pre-wrap text-destructive">
                          {stageRun.error_report}
                        </pre>
                      )}
                    </ScrollArea>
                  </TabsContent>

                  <TabsContent value="info" className="flex-1 p-4 m-0 overflow-hidden">
                    <ScrollArea className="h-full w-full">
                      <div className="space-y-4 text-sm">
                        <div className="grid grid-cols-2 gap-y-2">
                          <div className="text-muted-foreground">Task ID</div>
                          <div className="font-mono">{task?.id}</div>
                          
                          <div className="text-muted-foreground">Stage Run ID</div>
                          <div className="font-mono">{stageRun?.id || "-"}</div>
                          
                          <div className="text-muted-foreground">Workspace Path</div>
                          <div className="font-mono text-xs">{stageRun?.workspace_path || "-"}</div>
                          
                          <div className="text-muted-foreground">Branch</div>
                          <div className="font-mono">{stageRun?.branch || "-"}</div>
                        </div>

                        {stageRun?.prompt_used && (
                          <div className="mt-4 border-t pt-4">
                            <h4 className="font-medium mb-2">Prompt Used</h4>
                            <PromptViewer prompt={stageRun.prompt_used} />
                          </div>
                        )}
                      </div>
                    </ScrollArea>
                  </TabsContent>
                </Tabs>
              </CardContent>
            </Card>
          )}
        </div>
      </div>
    </div>
  );
}