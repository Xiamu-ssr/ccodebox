"use client";

import { useEffect, useState, useCallback } from "react";
import type { Template, CreateTemplateRequest, UpdateTemplateRequest } from "@/lib/types.generated";
import { listTemplates, createTemplate, updateTemplate, deleteTemplate } from "@/lib/api";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from "@/components/ui/card";
import { Button, buttonVariants } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/ui/badge";
import { IconPlus, IconFileCode, IconEdit, IconTrash, IconEye, IconPlayerPlay, IconLock } from "@tabler/icons-react";
import { cn } from "@/lib/utils";

export default function TemplatesPage() {
  const [templates, setTemplates] = useState<Template[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [editingTemplate, setEditingTemplate] = useState<Template | null>(null);
  const [viewingTemplate, setViewingTemplate] = useState<Template | null>(null);

  const fetchTemplates = useCallback(async () => {
    try {
      const res = await listTemplates();
      setTemplates(res.templates);
    } catch (err) {
      console.error("Failed to fetch templates:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchTemplates();
  }, [fetchTemplates]);

  async function handleDelete(e: React.MouseEvent, name: string) {
    e.preventDefault();
    e.stopPropagation();
    if (!confirm("Delete this template?")) return;
    try {
      await deleteTemplate(name);
      fetchTemplates();
    } catch (err) {
      console.error("Failed to delete template:", err);
      alert(err instanceof Error ? err.message : "Failed to delete template");
    }
  }

  function handleEdit(t: Template) {
    setEditingTemplate(t);
    setShowForm(true);
  }

  function handleView(t: Template) {
    setViewingTemplate(t);
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Templates</h1>
          <p className="text-muted-foreground mt-1">
            Manage task orchestration workflows
          </p>
        </div>
        
        <Dialog open={showForm} onOpenChange={(open) => {
          setShowForm(open);
          if (!open) setEditingTemplate(null);
        }}>
          <DialogTrigger className={cn(buttonVariants({ variant: "default" }))}>
            <IconPlus className="w-4 h-4 mr-2" />
            New Template
          </DialogTrigger>
          <DialogContent className="max-w-3xl max-h-[90vh] overflow-y-auto">
            <DialogHeader>
              <DialogTitle>{editingTemplate ? "Edit Template" : "Create New Template"}</DialogTitle>
            </DialogHeader>
            <TemplateForm
              initialData={editingTemplate}
              onSaved={() => {
                setShowForm(false);
                setEditingTemplate(null);
                fetchTemplates();
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {/* View Template Dialog */}
      <Dialog open={!!viewingTemplate} onOpenChange={(open) => !open && setViewingTemplate(null)}>
        <DialogContent className="max-w-3xl max-h-[90vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <IconFileCode className="h-5 w-5 text-primary" />
              {viewingTemplate?.name}
              {viewingTemplate?.builtin && (
                <Badge variant="secondary" className="ml-2"><IconLock className="h-3 w-3 mr-1" /> Builtin</Badge>
              )}
            </DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div>
              <h3 className="text-sm font-medium mb-1">Description</h3>
              <p className="text-sm text-muted-foreground">{viewingTemplate?.description}</p>
            </div>
            <div>
              <h3 className="text-sm font-medium mb-2">Definition (YAML)</h3>
              <div className="bg-muted rounded-md p-4 overflow-x-auto">
                <pre className="text-sm font-mono text-muted-foreground">
                  {viewingTemplate?.definition}
                </pre>
              </div>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      {loading ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {[1, 2, 3].map((i) => (
            <Card key={i}>
              <CardHeader className="space-y-2">
                <Skeleton className="h-5 w-1/2" />
                <Skeleton className="h-4 w-4/5" />
              </CardHeader>
              <CardContent>
                <Skeleton className="h-24 w-full" />
              </CardContent>
            </Card>
          ))}
        </div>
      ) : templates.length === 0 ? (
        <div className="text-center py-20 border rounded-lg border-dashed">
          <IconFileCode className="mx-auto h-12 w-12 text-muted-foreground opacity-50 mb-4" />
          <h3 className="text-lg font-medium">No templates yet</h3>
          <p className="text-sm text-muted-foreground mt-2 mb-4">
            Create your first template to define custom workflows.
          </p>
          <Button onClick={() => setShowForm(true)} variant="outline">
            <IconPlus className="w-4 h-4 mr-2" />
            Add Template
          </Button>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {templates.map((t) => (
            <Card key={t.id} className="h-full flex flex-col hover:border-primary/50 transition-colors">
              <CardHeader className="pb-3 flex-none">
                <div className="flex justify-between items-start">
                  <div className="flex items-center gap-2 min-w-0 pr-2">
                    <CardTitle className="text-xl truncate" title={t.name}>
                      {t.name}
                    </CardTitle>
                    {t.builtin && (
                      <Badge variant="secondary" className="flex-shrink-0 text-[10px] px-1.5 py-0">
                        <IconLock className="h-3 w-3 mr-1" /> Builtin
                      </Badge>
                    )}
                  </div>
                </div>
                <CardDescription className="line-clamp-2 mt-1.5" title={t.description}>
                  {t.description}
                </CardDescription>
              </CardHeader>
              <CardContent className="flex-1">
                <div className="bg-muted/50 rounded-md p-3 flex items-center justify-center h-full min-h-[80px]">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground font-mono">
                    <IconPlayerPlay className="h-3 w-3" />
                    {t.name === "single-stage" ? "[execute]" : "[coding] → [testing]"}
                  </div>
                </div>
              </CardContent>
              <CardFooter className="pt-0 flex justify-between gap-2">
                <Button variant="outline" size="sm" className="w-full" onClick={() => handleView(t)}>
                  <IconEye className="h-4 w-4 mr-2" /> View
                </Button>
                <Button variant="outline" size="sm" className="w-full" onClick={() => handleEdit(t)}>
                  <IconEdit className="h-4 w-4 mr-2" /> Edit
                </Button>
                {!t.builtin && (
                  <Button variant="outline" size="sm" className="w-full text-destructive hover:bg-destructive/10" onClick={(e) => handleDelete(e, t.name)}>
                    <IconTrash className="h-4 w-4 mr-2" /> Delete
                  </Button>
                )}
              </CardFooter>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}

function TemplateForm({ initialData, onSaved }: { initialData: Template | null, onSaved: () => void }) {
  const [name, setName] = useState(initialData?.name || "");
  const [description, setDescription] = useState(initialData?.description || "");
  const [definition, setDefinition] = useState(initialData?.definition || "name: my-template\ndescription: My template\ninputs:\n  - name: prompt\n    description: What to do\n    required: true\nstages:\n  - name: coding\n    agent: claude-code");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isBuiltin = initialData?.builtin || false;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim() || !definition.trim()) return;
    
    setSubmitting(true);
    setError(null);
    try {
      if (initialData) {
        const req: UpdateTemplateRequest = {
          description: description.trim() || null,
          definition: isBuiltin ? null : definition.trim(), // Can't update definition of builtin
        };
        await updateTemplate(initialData.name, req);
      } else {
        const req: CreateTemplateRequest = {
          name: name.trim(),
          description: description.trim(),
          definition: definition.trim(),
        };
        await createTemplate(req);
      }
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save template");
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
      
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="name">Template Name <span className="text-destructive">*</span></Label>
            <Input
              id="name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. feature-dev"
              required
              disabled={!!initialData} // Name cannot be changed after creation
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="description">Description</Label>
            <Input
              id="description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Brief description of the workflow"
            />
          </div>
        </div>

        <div className="space-y-2 md:col-span-2">
          <div className="flex items-center justify-between">
            <Label htmlFor="definition">YAML Definition <span className="text-destructive">*</span></Label>
            {isBuiltin && <span className="text-xs text-muted-foreground">Builtin template definition is read-only</span>}
          </div>
          <Textarea
            id="definition"
            value={definition}
            onChange={(e) => setDefinition(e.target.value)}
            placeholder="name: ..."
            required
            className="font-mono text-sm min-h-[300px]"
            disabled={isBuiltin}
          />
        </div>
      </div>

      <div className="pt-4 flex justify-end">
        <Button
          type="submit"
          disabled={submitting || !name.trim() || (!isBuiltin && !definition.trim())}
          className="w-full sm:w-auto"
        >
          {submitting ? "Saving..." : "Save Template"}
        </Button>
      </div>
    </form>
  );
}