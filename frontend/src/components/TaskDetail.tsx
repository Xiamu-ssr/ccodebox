"use client";

import { useState, useEffect, useCallback } from "react";
import { useRouter } from "next/navigation";
import type { Task, StageRun } from "@/lib/types.generated";
import { getTask, getTaskStages, cancelTask, stopStageRun } from "@/lib/api";
import StatusBadge from "./StatusBadge";
import DiffViewer from "./DiffViewer";
import MarkdownViewer from "./MarkdownViewer";
import AgentLogViewer from "./AgentLogViewer";
import PromptViewer from "./PromptViewer";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { ScrollArea } from "@/components/ui/scroll-area";
import { 
  IconArrowLeft, 
  IconClock, 
  IconCalendar, 
  IconBan, 
  IconTerminal2, 
  IconFileCode, 
  IconAlignLeft, 
  IconAlertCircle,
  IconChevronDown,
  IconChevronUp,
  IconCheck,
  IconX,
  IconCircleDashed,
  IconLoader2,
  IconBox,
  IconRobot,
  IconGitBranch,
  IconEye,
  IconEyeOff,
  IconTool
} from "@tabler/icons-react";
import { cn } from "@/lib/utils";

type Tab = "overview" | "stages" | "diff" | "summary";

function formatTime(iso: string | null): string {
  if (!iso) return "-";
  return new Date(iso).toLocaleString();
}

function duration(start: string | null, end: string | null): string {
  if (!start) return "-";
  const s = new Date(start).getTime();
  const e = end ? new Date(end).getTime() : Date.now();
  const sec = Math.floor((e - s) / 1000);
  if (sec < 60) return `${sec}s`;
  const min = Math.floor(sec / 60);
  return `${min}m ${sec % 60}s`;
}

export default function TaskDetail({ projectId, taskId }: { projectId?: string, taskId: string }) {
  const router = useRouter();
  const [task, setTask] = useState<Task | null>(null);
  const [stages, setStages] = useState<StageRun[]>([]);
  const [loading, setLoading] = useState(true);
  const [cancelling, setCancelling] = useState(false);

  const fetchData = useCallback(async () => {
    try {
      const [t, s] = await Promise.all([
        getTask(taskId),
        getTaskStages(taskId),
      ]);
      setTask(t);
      setStages(s);
    } catch (err) {
      console.error("Failed to fetch task:", err);
    } finally {
      setLoading(false);
    }
  }, [taskId]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  useEffect(() => {
    if (!task || task.status !== "running") return;
    const interval = setInterval(fetchData, 3000);
    return () => clearInterval(interval);
  }, [task, fetchData]);

  async function handleCancel() {
    if (!task || cancelling) return;
    setCancelling(true);
    try {
      await cancelTask(task.id);
      fetchData();
    } catch (err) {
      console.error("Failed to cancel:", err);
    } finally {
      setCancelling(false);
    }
  }

  if (loading) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-10 w-32" />
        <Card>
          <CardHeader>
            <Skeleton className="h-8 w-1/3 mb-2" />
            <Skeleton className="h-4 w-1/4" />
          </CardHeader>
        </Card>
        <Skeleton className="h-[400px] w-full" />
      </div>
    );
  }

  if (!task) {
    return (
      <div className="text-center py-20 border rounded-lg border-dashed">
        <IconAlertCircle className="mx-auto h-12 w-12 text-muted-foreground opacity-50 mb-4" />
        <h2 className="text-xl font-bold mb-2">Task not found</h2>
        <Button onClick={() => router.push(projectId ? `/projects/${projectId}` : "/projects")} variant="outline">
          Go Back
        </Button>
      </div>
    );
  }

  const isRunning = task.status === "running";

  const aggregatedDiff = stages
    .filter((s) => s.diff_patch)
    .map((s) => s.diff_patch!)
    .join("\n");

  const aggregatedSummary = stages
    .filter((s) => s.summary)
    .map((s) => `## ${s.stage_name} (run #${s.run_number})\n\n${s.summary}`)
    .join("\n\n---\n\n");

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
        <div className="flex items-center gap-4">
          <Button variant="ghost" size="icon" onClick={() => router.push(projectId ? `/projects/${projectId}` : "/projects")}>
            <IconArrowLeft className="h-5 w-5" />
          </Button>
          <div>
            <div className="flex items-center gap-3">
              <h1 className="text-2xl font-bold tracking-tight">{task.title}</h1>
              <StatusBadge status={task.status} />
            </div>
            <div className="flex flex-wrap items-center gap-x-4 gap-y-2 mt-2 text-sm text-muted-foreground">
              <span className="flex items-center">
                <IconBox className="h-3.5 w-3.5 mr-1.5" />
                <span className="font-mono">{task.task_type}</span>
              </span>
              <span className="flex items-center">
                <IconCalendar className="h-3.5 w-3.5 mr-1.5" />
                {formatTime(task.created_at)}
              </span>
              {(task.started_at || task.finished_at) && (
                <span className="flex items-center">
                  <IconClock className="h-3.5 w-3.5 mr-1.5" />
                  {duration(task.started_at, task.finished_at)}
                </span>
              )}
            </div>
          </div>
        </div>
        
        {isRunning && (
          <Button
            variant="destructive"
            onClick={handleCancel}
            disabled={cancelling}
          >
            <IconBan className="w-4 h-4 mr-2" />
            {cancelling ? "Cancelling..." : "Cancel Task"}
          </Button>
        )}
      </div>

      {/* Error */}
      {task.error && (
        <div className="bg-destructive/10 border border-destructive/30 text-destructive rounded-md p-4 text-sm font-mono flex items-start">
          <IconAlertCircle className="h-5 w-5 mr-3 flex-shrink-0 mt-0.5" />
          <div className="whitespace-pre-wrap">{task.error}</div>
        </div>
      )}

      {/* Main Content Tabs */}
      <Tabs defaultValue="stages" className="w-full">
        <TabsList className="flex w-full sm:w-auto border-b rounded-none bg-transparent h-auto p-0 gap-6">
          <TabsTrigger 
            value="stages" 
            className="rounded-none border-b-2 border-transparent data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:shadow-none py-2 px-1"
          >
            Stages ({stages.length})
          </TabsTrigger>
          <TabsTrigger 
            value="overview" 
            className="rounded-none border-b-2 border-transparent data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:shadow-none py-2 px-1"
          >
            Overview
          </TabsTrigger>
        </TabsList>
        
        <div className="mt-6">
          <TabsContent value="overview" className="m-0">
            <OverviewTab task={task} />
          </TabsContent>
          
          <TabsContent value="stages" className="m-0">
            <StagesTab stages={stages} taskStatus={task.status} onRefresh={fetchData} />
          </TabsContent>
        </div>
      </Tabs>
    </div>
  );
}

function OverviewTab({ task }: { task: Task }) {
  let inputs: Record<string, string> = {};
  if (task.inputs) {
    try {
      inputs = JSON.parse(task.inputs);
    } catch {
      /* ignore */
    }
  }

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
      <Card className="md:col-span-2">
        <CardHeader className="pb-3">
          <CardTitle className="text-lg flex items-center gap-2">
            <IconAlignLeft className="h-5 w-5" />
            Requirement Prompt
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="bg-muted/50 rounded-md p-4 font-mono text-sm whitespace-pre-wrap break-words border">
            {task.prompt}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-base">Configuration</CardTitle>
        </CardHeader>
        <CardContent>
          <dl className="space-y-4 text-sm">
            <InfoRow label="Task Type" value={task.task_type} mono />
            {task.current_stage && (
              <InfoRow label="Current Stage" value={task.current_stage} mono />
            )}
            {task.project_id && (
              <InfoRow label="Project ID" value={task.project_id} mono />
            )}
            <InfoRow label="Task ID" value={task.id} mono />
          </dl>
        </CardContent>
      </Card>

      {Object.keys(inputs).length > 0 && (
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">Inputs</CardTitle>
          </CardHeader>
          <CardContent>
            <dl className="space-y-4 text-sm">
              {Object.entries(inputs)
                .filter(([key]) => key !== 'prompt' && key !== 'requirement')
                .map(([key, val]) => (
                <InfoRow key={key} label={key} value={val} mono />
              ))}
            </dl>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function StageIcon({ status }: { status: string }) {
  switch (status) {
    case "success": return <IconCheck className="h-5 w-5 text-green-500" />;
    case "failed": return <IconX className="h-5 w-5 text-destructive" />;
    case "running": return <IconLoader2 className="h-5 w-5 text-blue-500 animate-spin" />;
    default: return <IconCircleDashed className="h-5 w-5 text-muted-foreground" />;
  }
}

function StagesTab({ 
  stages, 
  taskStatus, 
  onRefresh 
}: { 
  stages: StageRun[], 
  taskStatus: string, 
  onRefresh: () => void 
}) {
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [stoppingIds, setStoppingIds] = useState<Set<string>>(new Set());

  // Auto-expand the last stage if task is running
  useEffect(() => {
    if (taskStatus === "running" && stages.length > 0 && !expandedId) {
      setExpandedId(stages[stages.length - 1].id);
    }
  }, [taskStatus, stages, expandedId]);

  async function handleStopStage(e: React.MouseEvent, id: string) {
    e.stopPropagation();
    if (stoppingIds.has(id)) return;
    
    setStoppingIds(prev => new Set(prev).add(id));
    try {
      await stopStageRun(id);
      onRefresh();
    } catch (err) {
      console.error("Failed to stop stage run:", err);
    } finally {
      setStoppingIds(prev => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  }

  if (stages.length === 0) {
    return (
      <div className="text-center py-20 border rounded-lg border-dashed">
        <IconBox className="mx-auto h-12 w-12 text-muted-foreground opacity-50 mb-4" />
        <h3 className="text-lg font-medium">No stages executed yet</h3>
        <p className="text-sm text-muted-foreground mt-1">
          Stages will appear here once the task starts running.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {stages.map((sr, index) => {
        const expanded = expandedId === sr.id;
        
        return (
          <div key={sr.id} className="w-full">
            <Card className={cn(
              "w-full transition-all overflow-hidden",
              expanded ? "ring-1 ring-primary/20 border-primary/50 shadow-md" : "hover:border-primary/30"
            )}>
              <div 
                className="p-4 cursor-pointer flex items-center justify-between bg-card hover:bg-muted/30 transition-colors"
                onClick={() => setExpandedId(expanded ? null : sr.id)}
              >
                <div className="flex items-center gap-3">
                  <div className="flex items-center justify-center w-8 h-8 rounded-full border bg-background shrink-0">
                    <StageIcon status={sr.status} />
                  </div>
                  <div className="flex flex-col gap-1">
                    <div className="flex items-center gap-2">
                      <span className="font-semibold">{sr.stage_name}</span>
                      <Badge variant="outline" className="text-[10px] h-5 px-1.5 py-0 font-mono">
                        run #{sr.run_number}
                      </Badge>
                      {sr.status === "running" && (
                        <Button
                          variant="destructive"
                          size="xs"
                          className="h-5 px-2 text-[10px] uppercase font-bold"
                          onClick={(e) => handleStopStage(e, sr.id)}
                          disabled={stoppingIds.has(sr.id)}
                        >
                          {stoppingIds.has(sr.id) ? (
                            <IconLoader2 className="h-3 w-3 animate-spin mr-1" />
                          ) : (
                            <IconX className="h-3 w-3 mr-1" />
                          )}
                          Stop
                        </Button>
                      )}
                    </div>
                    <div className="flex items-center gap-3 text-xs text-muted-foreground">
                      {sr.duration_seconds != null && (
                        <span className="flex items-center"><IconClock className="h-3 w-3 mr-1" />{sr.duration_seconds}s</span>
                      )}
                      {sr.agent_exit_code != null && (
                        <span className={cn(
                          "flex items-center",
                          sr.agent_exit_code === 0 ? "text-green-600 dark:text-green-400" : "text-destructive"
                        )}>
                          exit {sr.agent_exit_code}
                        </span>
                      )}
                    </div>
                  </div>
                </div>
                <div>
                  {expanded ? <IconChevronUp className="h-5 w-5 text-muted-foreground" /> : <IconChevronDown className="h-5 w-5 text-muted-foreground" />}
                </div>
              </div>

              {expanded && (
                <div className="border-t bg-muted/10 p-0">
                  <Tabs defaultValue="log" className="w-full">
                    <div className="px-4 pt-3 border-b bg-muted/20">
                      <TabsList className="h-8 mb-3 bg-background border">
                        <TabsTrigger value="log" className="text-xs h-6 px-3">
                          <IconTerminal2 className="h-3 w-3 mr-1.5" /> Log
                        </TabsTrigger>
                        <TabsTrigger value="diff" className="text-xs h-6 px-3" disabled={!sr.diff_patch}>
                          <IconFileCode className="h-3 w-3 mr-1.5" /> Diff
                        </TabsTrigger>
                        <TabsTrigger value="summary" className="text-xs h-6 px-3" disabled={!sr.summary}>
                          <IconAlignLeft className="h-3 w-3 mr-1.5" /> Summary
                        </TabsTrigger>
                        <TabsTrigger value="prompt" className="text-xs h-6 px-3">
                          Prompt
                        </TabsTrigger>
                      </TabsList>
                    </div>

                    <div className="p-4">
                      {/* Meta Info */}
                      <div className="flex flex-wrap gap-x-4 gap-y-2 text-xs text-muted-foreground mb-4 bg-muted/30 p-2 rounded-md border">
                        <span className="flex items-center"><IconRobot className="h-3.5 w-3.5 mr-1.5" /> {sr.agent_type}</span>
                        {sr.branch && <span className="flex items-center"><IconGitBranch className="h-3.5 w-3.5 mr-1.5" /> {sr.branch}</span>}
                      </div>

                      {sr.error_report && (
                        <div className="mb-4 bg-destructive/10 border border-destructive/30 rounded-md p-3">
                          <h4 className="text-xs font-semibold text-destructive flex items-center mb-2">
                            <IconAlertCircle className="h-3.5 w-3.5 mr-1.5" /> Error Report
                          </h4>
                          <pre className="text-xs font-mono whitespace-pre-wrap break-words text-destructive/90 overflow-x-auto">
                            {sr.error_report}
                          </pre>
                        </div>
                      )}

                      <TabsContent value="log" className="m-0">
                        <AgentLogViewer log={sr.agent_log} status={sr.status} />
                      </TabsContent>

                      <TabsContent value="diff" className="m-0">
                        {sr.diff_patch && <DiffViewer diff={sr.diff_patch} />}
                      </TabsContent>

                      <TabsContent value="summary" className="m-0 prose prose-sm dark:prose-invert max-w-none">
                        {sr.summary && <MarkdownViewer content={sr.summary} />}
                      </TabsContent>

                      <TabsContent value="prompt" className="m-0">
                        <PromptViewer prompt={sr.prompt_used} />
                      </TabsContent>
                    </div>
                  </Tabs>
                </div>
              )}
            </Card>
          </div>
        );
      })}
    </div>
  );
}

function InfoRow({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="flex justify-between items-center py-1 border-b border-border/50 last:border-0">
      <dt className="text-muted-foreground font-medium">{label}</dt>
      <dd className={cn("text-foreground", mono && "font-mono text-xs bg-muted px-1.5 py-0.5 rounded")}>
        {value}
      </dd>
    </div>
  );
}
