"use client";

import { useEffect, useState, useCallback } from "react";
import type {
  Template,
  CreateTemplateRequest,
  UpdateTemplateRequest,
} from "@/lib/types.generated";
import {
  listTemplates,
  createTemplate,
  updateTemplate,
  deleteTemplate,
} from "@/lib/api";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from "@/components/ui/card";
import { Button, buttonVariants } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  IconPlus,
  IconFileCode,
  IconEdit,
  IconTrash,
  IconEye,
  IconPlayerPlay,
  IconLock,
} from "@tabler/icons-react";
import { cn } from "@/lib/utils";

const TEMPLATE_FORM_DIALOG_CLASSNAME =
  "grid h-[min(94vh,54rem)] w-[min(96vw,88rem)] max-w-[min(96vw,88rem)] sm:max-w-[min(96vw,88rem)] grid-rows-[auto_minmax(0,1fr)] gap-0 overflow-hidden p-0";

const TEMPLATE_VIEW_DIALOG_CLASSNAME =
  "grid h-[min(94vh,52rem)] w-[min(96vw,84rem)] max-w-[min(96vw,84rem)] sm:max-w-[min(96vw,84rem)] grid-rows-[auto_minmax(0,1fr)] gap-0 overflow-hidden p-0";

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

  function handleEdit(template: Template) {
    setEditingTemplate(template);
    setShowForm(true);
  }

  function handleView(template: Template) {
    setViewingTemplate(template);
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Templates</h1>
          <p className="mt-1 text-muted-foreground">
            Manage task orchestration workflows
          </p>
        </div>

        <Dialog
          open={showForm}
          onOpenChange={(open) => {
            setShowForm(open);
            if (!open) setEditingTemplate(null);
          }}
        >
          <DialogTrigger className={cn(buttonVariants({ variant: "default" }))}>
            <IconPlus className="mr-2 h-4 w-4" />
            New Template
          </DialogTrigger>
          <DialogContent className={TEMPLATE_FORM_DIALOG_CLASSNAME}>
            <DialogHeader className="border-b px-6 py-5 pr-14">
              <DialogTitle>
                {editingTemplate ? "Edit Template" : "Create New Template"}
              </DialogTitle>
              <DialogDescription>
                Work with a larger YAML workspace so long stage definitions,
                retries, and inputs stay readable while editing.
              </DialogDescription>
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

      <Dialog
        open={!!viewingTemplate}
        onOpenChange={(open) => !open && setViewingTemplate(null)}
      >
        <DialogContent className={TEMPLATE_VIEW_DIALOG_CLASSNAME}>
          <DialogHeader className="border-b px-6 py-5 pr-14">
            <DialogTitle className="flex items-center gap-2">
              <IconFileCode className="h-5 w-5 text-primary" />
              {viewingTemplate?.name}
              {viewingTemplate?.builtin && (
                <Badge variant="secondary" className="ml-2">
                  <IconLock className="mr-1 h-3 w-3" />
                  Builtin
                </Badge>
              )}
            </DialogTitle>
            <DialogDescription>
              Inspect the template metadata on the left and the full YAML
              definition in a dedicated code pane.
            </DialogDescription>
          </DialogHeader>

          <div className="grid min-h-0 grid-cols-1 md:grid-cols-[20rem_minmax(0,1fr)]">
            <div className="border-b bg-muted/20 p-5 md:border-r md:border-b-0">
              <div className="space-y-5">
                <div className="space-y-1">
                  <h3 className="text-sm font-medium">Description</h3>
                  <p className="text-sm leading-6 text-muted-foreground">
                    {viewingTemplate?.description || "No description provided."}
                  </p>
                </div>

                <div className="grid gap-3 sm:grid-cols-2 md:grid-cols-1">
                  <InfoTile
                    label="Template ID"
                    value={viewingTemplate?.id ?? "-"}
                    mono
                  />
                  <InfoTile
                    label="Updated At"
                    value={formatTimestamp(viewingTemplate?.updated_at)}
                  />
                  <InfoTile
                    label="Created At"
                    value={formatTimestamp(viewingTemplate?.created_at)}
                  />
                  <InfoTile
                    label="Definition Size"
                    value={`${viewingTemplate?.definition.split("\n").length ?? 0} lines`}
                  />
                </div>
              </div>
            </div>

            <div className="flex min-h-0 flex-col p-5">
              <div className="mb-3 flex items-center justify-between gap-3">
                <div>
                  <h3 className="text-sm font-medium">Definition (YAML)</h3>
                  <p className="text-xs text-muted-foreground">
                    Independent scrolling keeps large templates readable without
                    shrinking the code pane.
                  </p>
                </div>
                <Badge variant="outline" className="font-mono">
                  {viewingTemplate?.definition.length ?? 0} chars
                </Badge>
              </div>

              <ScrollArea className="min-h-0 flex-1 rounded-xl border bg-muted/30">
                <pre className="min-w-max p-5 font-mono text-sm leading-6 whitespace-pre text-foreground">
                  {viewingTemplate?.definition}
                </pre>
              </ScrollArea>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      {loading ? (
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
          {[1, 2, 3].map((index) => (
            <Card key={index}>
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
        <div className="rounded-lg border border-dashed py-20 text-center">
          <IconFileCode className="mx-auto mb-4 h-12 w-12 text-muted-foreground opacity-50" />
          <h3 className="text-lg font-medium">No templates yet</h3>
          <p className="mt-2 mb-4 text-sm text-muted-foreground">
            Create your first template to define custom workflows.
          </p>
          <Button onClick={() => setShowForm(true)} variant="outline">
            <IconPlus className="mr-2 h-4 w-4" />
            Add Template
          </Button>
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
          {templates.map((template) => (
            <Card
              key={template.id}
              className="flex h-full flex-col transition-colors hover:border-primary/50"
            >
              <CardHeader className="flex-none pb-3">
                <div className="flex items-start justify-between">
                  <div className="flex min-w-0 items-center gap-2 pr-2">
                    <CardTitle className="truncate text-xl" title={template.name}>
                      {template.name}
                    </CardTitle>
                    {template.builtin && (
                      <Badge
                        variant="secondary"
                        className="flex-shrink-0 px-1.5 py-0 text-[10px]"
                      >
                        <IconLock className="mr-1 h-3 w-3" />
                        Builtin
                      </Badge>
                    )}
                  </div>
                </div>
                <CardDescription
                  className="mt-1.5 line-clamp-2"
                  title={template.description}
                >
                  {template.description}
                </CardDescription>
              </CardHeader>
              <CardContent className="flex-1">
                <div className="flex h-full min-h-[80px] items-center justify-center rounded-md bg-muted/50 p-3">
                  <div className="flex items-center gap-2 font-mono text-xs text-muted-foreground">
                    <IconPlayerPlay className="h-3 w-3" />
                    {template.name === "single-stage"
                      ? "[execute]"
                      : "[coding] → [testing]"}
                  </div>
                </div>
              </CardContent>
              <CardFooter className="justify-between gap-2 pt-0">
                <Button
                  variant="outline"
                  size="sm"
                  className="w-full"
                  onClick={() => handleView(template)}
                >
                  <IconEye className="mr-2 h-4 w-4" />
                  View
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  className="w-full"
                  onClick={() => handleEdit(template)}
                >
                  <IconEdit className="mr-2 h-4 w-4" />
                  Edit
                </Button>
                {!template.builtin && (
                  <Button
                    variant="outline"
                    size="sm"
                    className="w-full text-destructive hover:bg-destructive/10"
                    onClick={(event) => handleDelete(event, template.name)}
                  >
                    <IconTrash className="mr-2 h-4 w-4" />
                    Delete
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

function TemplateForm({
  initialData,
  onSaved,
}: {
  initialData: Template | null;
  onSaved: () => void;
}) {
  const [name, setName] = useState(initialData?.name || "");
  const [description, setDescription] = useState(
    initialData?.description || ""
  );
  const [definition, setDefinition] = useState(
    initialData?.definition ||
      "name: my-template\ndescription: My template\ninputs:\n  - name: prompt\n    description: What to do\n    required: true\nstages:\n  - name: coding\n    agent: claude-code"
  );
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
          definition: isBuiltin ? null : definition.trim(),
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
    <form
      onSubmit={handleSubmit}
      className="grid min-h-0 grid-cols-1 grid-rows-[auto_minmax(0,1fr)_auto] gap-0 md:grid-cols-[20rem_minmax(0,1fr)] md:grid-rows-[minmax(0,1fr)_auto]"
    >
      {error && (
        <div className="border-b bg-destructive/10 px-6 py-3 text-sm text-destructive md:col-span-2">
          {error}
        </div>
      )}

      <div className="border-b bg-muted/20 p-5 md:border-r md:border-b-0">
        <div className="space-y-5">
          <div className="space-y-2">
            <Label htmlFor="name">
              Template Name <span className="text-destructive">*</span>
            </Label>
            <Input
              id="name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. feature-dev"
              required
              disabled={!!initialData}
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

          <div className="rounded-xl border bg-background/80 p-4">
            <h3 className="text-sm font-medium">Writing Tips</h3>
            <ul className="mt-3 space-y-2 text-sm leading-6 text-muted-foreground">
              <li>
                Keep metadata in the left column and use the larger pane for the
                YAML itself.
              </li>
              <li>
                Declare inputs first, then define stages in execution order.
              </li>
              <li>
                The editor stays large on desktop so long workflows remain
                practical to author.
              </li>
            </ul>
          </div>
        </div>
      </div>

      <div className="flex min-h-0 flex-col p-5">
        <div className="mb-3 flex items-center justify-between gap-3">
          <div>
            <Label htmlFor="definition">
              YAML Definition <span className="text-destructive">*</span>
            </Label>
            <p className="mt-1 text-xs text-muted-foreground">
              {isBuiltin
                ? "Builtin template definitions are read-only."
                : "Use the expanded editor below for multi-stage templates and long retry rules."}
            </p>
          </div>
          <Badge variant="outline" className="font-mono">
            {definition.split("\n").length} lines
          </Badge>
        </div>

        <Textarea
          id="definition"
          value={definition}
          onChange={(e) => setDefinition(e.target.value)}
          placeholder="name: ..."
          required
          disabled={isBuiltin}
          spellCheck={false}
          wrap="off"
          className="min-h-[24rem] flex-1 resize-none rounded-xl border bg-background font-mono text-sm leading-6"
        />
      </div>

      <div className="flex flex-col gap-3 border-t bg-muted/20 px-5 py-4 md:col-span-2 md:flex-row md:items-center md:justify-between">
        <p className="text-sm text-muted-foreground">
          {initialData
            ? "Changes save back to the existing template."
            : "New templates become available immediately after save."}
        </p>
        <Button
          type="submit"
          disabled={submitting || !name.trim() || (!isBuiltin && !definition.trim())}
          className="w-full md:w-auto"
        >
          {submitting ? "Saving..." : "Save Template"}
        </Button>
      </div>
    </form>
  );
}

function InfoTile({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="rounded-xl border bg-background/80 p-3">
      <div className="text-xs uppercase tracking-wide text-muted-foreground">
        {label}
      </div>
      <div
        className={cn(
          "mt-1 text-sm text-foreground",
          mono && "font-mono break-all"
        )}
      >
        {value}
      </div>
    </div>
  );
}

function formatTimestamp(value?: string) {
  if (!value) return "-";
  return new Date(value).toLocaleString();
}
