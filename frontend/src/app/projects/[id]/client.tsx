"use client";

import { useEffect, useState, useCallback, use } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import type { Task, TaskStatus, Project } from "@/lib/types.generated";
import { listTasks, getProject } from "@/lib/api";
import TaskCard from "@/components/TaskCard";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/ui/badge";
import { IconArrowLeft, IconGitBranch, IconFolder, IconPlus, IconCalendar } from "@tabler/icons-react";

const STATUS_FILTERS: { label: string; value: TaskStatus | "all" }[] = [
  { label: "All", value: "all" },
  { label: "Running", value: "running" },
  { label: "Success", value: "success" },
  { label: "Failed", value: "failed" },
  { label: "Pending", value: "pending" },
  { label: "Cancelled", value: "cancelled" },
];

export default function ProjectDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const resolvedParams = use(params);
  const router = useRouter();
  const [project, setProject] = useState<Project | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [total, setTotal] = useState(0);
  const [filter, setFilter] = useState<TaskStatus | "all">("all");
  const [loading, setLoading] = useState(true);

  const fetchData = useCallback(async () => {
    try {
      const [taskRes, projRes] = await Promise.all([
        listTasks({
          status: filter === "all" ? undefined : filter,
          limit: 50,
        }),
        getProject(resolvedParams.id),
      ]);
      
      // Filter tasks by project since API might not support project_id filter yet
      const projectTasks = taskRes.tasks.filter(t => t.project_id === resolvedParams.id);
      
      setTasks(projectTasks);
      setTotal(projectTasks.length);
      setProject(projRes);
    } catch (err) {
      console.error("Failed to fetch data:", err);
    } finally {
      setLoading(false);
    }
  }, [filter, resolvedParams.id]);

  useEffect(() => {
    setLoading(true);
    fetchData();
  }, [fetchData]);

  // Poll for updates when there are running tasks
  useEffect(() => {
    const hasRunning = tasks.some((t) => t.status === "running");
    if (!hasRunning) return;

    const interval = setInterval(fetchData, 3000);
    return () => clearInterval(interval);
  }, [tasks, fetchData]);

  if (loading && !project) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-10 w-32" />
        <Card>
          <CardHeader>
            <Skeleton className="h-8 w-1/3 mb-2" />
            <Skeleton className="h-4 w-1/4" />
          </CardHeader>
        </Card>
      </div>
    );
  }

  if (!project) {
    return (
      <div className="text-center py-20">
        <h2 className="text-2xl font-bold mb-2">Project not found</h2>
        <Button onClick={() => router.push("/projects")} variant="outline">
          Back to Projects
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" onClick={() => router.push("/projects")}>
          <IconArrowLeft className="h-5 w-5" />
        </Button>
        <h1 className="text-2xl font-bold tracking-tight">Project Details</h1>
      </div>

      {/* Project Info Card */}
      <Card>
        <CardHeader className="pb-4">
          <div className="flex justify-between items-start">
            <div>
              <CardTitle className="text-2xl flex items-center gap-3">
                {project.name}
                {project.default_agent && (
                  <Badge variant="secondary" className="font-mono font-normal">
                    {project.default_agent}
                  </Badge>
                )}
              </CardTitle>
              <CardDescription className="flex items-center gap-4 mt-2">
                <span className="flex items-center text-sm">
                  <IconCalendar className="h-4 w-4 mr-1.5" />
                  {new Date(project.created_at).toLocaleDateString()}
                </span>
              </CardDescription>
            </div>
            <Button onClick={() => router.push(`/projects/${project.id}/tasks/new`)}>
              <IconPlus className="w-4 h-4 mr-2" />
              New Task
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <div className="flex flex-col sm:flex-row gap-4 sm:gap-8 text-sm">
            {project.local_path && (
              <div className="flex items-center gap-2 text-muted-foreground">
                <IconFolder className="h-4 w-4" />
                <span className="font-mono">{project.local_path}</span>
              </div>
            )}
            {project.repo_url && (
              <div className="flex items-center gap-2 text-muted-foreground">
                <IconGitBranch className="h-4 w-4" />
                <a href={project.repo_url} target="_blank" rel="noopener noreferrer" className="font-mono hover:text-primary transition-colors">
                  {project.repo_url}
                </a>
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Tasks Section */}
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-xl font-semibold tracking-tight">
            Tasks <span className="text-muted-foreground text-base font-normal ml-2">({total})</span>
          </h2>
          
          <div className="flex gap-2 overflow-x-auto pb-1">
            {STATUS_FILTERS.map((f) => (
              <Button
                key={f.value}
                variant={filter === f.value ? "default" : "outline"}
                size="sm"
                onClick={() => setFilter(f.value)}
                className="rounded-full"
              >
                {f.label}
              </Button>
            ))}
          </div>
        </div>

        {loading ? (
          <div className="space-y-3">
            {[1, 2, 3].map(i => <Skeleton key={i} className="h-24 w-full" />)}
          </div>
        ) : tasks.length === 0 ? (
          <div className="text-center py-16 border rounded-lg border-dashed">
            <h3 className="text-lg font-medium text-muted-foreground">No tasks found</h3>
            <p className="text-sm text-muted-foreground mt-1 mb-4">
              {filter === "all" ? "Create your first task to get started." : "Try changing the status filter."}
            </p>
            {filter === "all" && (
              <Button onClick={() => router.push(`/projects/${project.id}/tasks/new`)} variant="outline">
                <IconPlus className="w-4 h-4 mr-2" />
                New Task
              </Button>
            )}
          </div>
        ) : (
          <div className="grid gap-3">
            {tasks.map((task) => (
              <TaskCard
                key={task.id}
                task={task}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}