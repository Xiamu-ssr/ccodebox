"use client";

import { useEffect, useState, useCallback } from "react";
import Link from "next/link";
import type { Project, CreateProjectRequest } from "@/lib/types.generated";
import { listProjects, createProject, deleteProject } from "@/lib/api";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { Button, buttonVariants } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { IconTrash, IconPlus, IconFolder, IconGitBranch, IconLayoutDashboard } from "@tabler/icons-react";
import { cn } from "@/lib/utils";

export default function ProjectsPage() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);

  const fetchProjects = useCallback(async () => {
    try {
      const res = await listProjects();
      setProjects(res.projects);
    } catch (err) {
      console.error("Failed to fetch projects:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchProjects();
  }, [fetchProjects]);

  async function handleDelete(e: React.MouseEvent, id: string) {
    e.preventDefault();
    e.stopPropagation();
    if (!confirm("Delete this project?")) return;
    try {
      await deleteProject(id);
      fetchProjects();
    } catch (err) {
      console.error("Failed to delete project:", err);
    }
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Projects</h1>
          <p className="text-muted-foreground mt-1">
            Manage your workspaces and repositories
          </p>
        </div>
        
        <Dialog open={showForm} onOpenChange={setShowForm}>
          <DialogTrigger className={cn(buttonVariants({ variant: "default" }))}>
            <IconPlus className="w-4 h-4 mr-2" />
            New Project
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Create New Project</DialogTitle>
            </DialogHeader>
            <AddProjectForm
              onCreated={() => {
                setShowForm(false);
                fetchProjects();
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {loading ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {[1, 2, 3].map((i) => (
            <Card key={i}>
              <CardHeader className="space-y-2">
                <Skeleton className="h-5 w-1/2" />
                <Skeleton className="h-4 w-4/5" />
              </CardHeader>
              <CardContent>
                <Skeleton className="h-4 w-full" />
              </CardContent>
            </Card>
          ))}
        </div>
      ) : projects.length === 0 ? (
        <div className="text-center py-20 border rounded-lg border-dashed">
          <IconLayoutDashboard className="mx-auto h-12 w-12 text-muted-foreground opacity-50 mb-4" />
          <h3 className="text-lg font-medium">No projects yet</h3>
          <p className="text-sm text-muted-foreground mt-2 mb-4">
            Create your first project to start running tasks.
          </p>
          <Button onClick={() => setShowForm(true)} variant="outline">
            <IconPlus className="w-4 h-4 mr-2" />
            Add Project
          </Button>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {projects.map((p) => (
            <Link key={p.id} href={`/projects/${p.id}`} className="block group">
              <Card className="h-full transition-colors hover:border-primary/50 relative">
                <CardHeader className="pb-3">
                  <div className="flex justify-between items-start">
                    <CardTitle className="text-xl truncate pr-6 group-hover:text-primary transition-colors">
                      {p.name}
                    </CardTitle>
                    <Button 
                      variant="ghost" 
                      size="icon" 
                      className="absolute top-4 right-4 h-8 w-8 text-muted-foreground hover:text-destructive hover:bg-destructive/10 opacity-0 group-hover:opacity-100 transition-all"
                      onClick={(e) => handleDelete(e, p.id)}
                    >
                      <IconTrash className="h-4 w-4" />
                    </Button>
                  </div>
                  <CardDescription className="flex flex-col gap-1.5 mt-2">
                    {p.local_path && (
                      <span className="flex items-center text-xs truncate" title={p.local_path}>
                        <IconFolder className="h-3 w-3 mr-1.5 flex-shrink-0" />
                        <span className="truncate font-mono">{p.local_path}</span>
                      </span>
                    )}
                    {p.repo_url && (
                      <span className="flex items-center text-xs truncate" title={p.repo_url}>
                        <IconGitBranch className="h-3 w-3 mr-1.5 flex-shrink-0" />
                        <span className="truncate font-mono">{p.repo_url}</span>
                      </span>
                    )}
                  </CardDescription>
                </CardHeader>
                <CardContent>
                  <div className="text-xs text-muted-foreground flex justify-between items-center">
                    <span>Created {new Date(p.created_at).toLocaleDateString()}</span>
                  </div>
                </CardContent>
              </Card>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}

function AddProjectForm({ onCreated }: { onCreated: () => void }) {
  const [name, setName] = useState("");
  const [localPath, setLocalPath] = useState("");
  const [repoUrl, setRepoUrl] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) return;
    
    // In spec, backend automatically handles local_path if repo is provided
    setSubmitting(true);
    setError(null);
    try {
      const req: CreateProjectRequest = {
        name: name.trim(),
        local_path: localPath.trim() || null,
        repo_url: repoUrl.trim() || null,
        default_agent: null, // Spec doesn't require this in form
      };
      await createProject(req);
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create project");
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-4 pt-4">
      {error && (
        <div className="bg-destructive/10 border border-destructive/30 text-destructive rounded-md p-3 text-sm">
          {error}
        </div>
      )}
      
      <div className="space-y-2">
        <Label htmlFor="name">Project Name <span className="text-destructive">*</span></Label>
        <Input
          id="name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g. my-awesome-app"
          required
          autoFocus
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="repoUrl">GitHub URL <span className="text-muted-foreground font-normal text-xs">(Optional)</span></Label>
        <Input
          id="repoUrl"
          value={repoUrl}
          onChange={(e) => setRepoUrl(e.target.value)}
          placeholder="https://github.com/user/repo"
          className="font-mono text-sm"
        />
        <p className="text-xs text-muted-foreground">
          Backend will automatically clone and set local path if provided.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="localPath">Local Path <span className="text-muted-foreground font-normal text-xs">(Optional)</span></Label>
        <Input
          id="localPath"
          value={localPath}
          onChange={(e) => setLocalPath(e.target.value)}
          placeholder="/Users/name/projects/app"
          className="font-mono text-sm"
        />
      </div>

      <div className="pt-4 flex justify-end">
        <Button
          type="submit"
          disabled={submitting || !name.trim()}
          className="w-full sm:w-auto"
        >
          {submitting ? "Creating..." : "Create Project"}
        </Button>
      </div>
    </form>
  );
}
