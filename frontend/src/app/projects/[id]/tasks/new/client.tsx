"use client";

import { useState, useEffect, use } from "react";
import { useRouter } from "next/navigation";
import type { Project, TaskTypeInfo, AgentInfo } from "@/lib/types.generated";
import { createTask, getProject, listTaskTypes, getAgents } from "@/lib/api";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Skeleton } from "@/components/ui/skeleton";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { IconArrowLeft, IconCheck, IconChevronRight, IconPlayerPlay, IconBox, IconRobot } from "@tabler/icons-react";
import { cn } from "@/lib/utils";

export default function NewTaskPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const resolvedParams = use(params);
  const router = useRouter();
  const [project, setProject] = useState<Project | null>(null);
  const [taskTypes, setTaskTypes] = useState<TaskTypeInfo[]>([]);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Step state
  const [step, setStep] = useState<1 | 2 | 3>(1);

  // Form state
  const [taskType, setTaskType] = useState("");
  const [title, setTitle] = useState("");
  const [inputValues, setInputValues] = useState<Record<string, string>>({});
  
  // Advanced options
  const [agentType, setAgentType] = useState("");
  const [model, setModel] = useState("");
  const [showAdvanced, setShowAdvanced] = useState(false);

  useEffect(() => {
    Promise.all([
      getProject(resolvedParams.id),
      listTaskTypes(),
      getAgents()
    ])
      .then(([projRes, ttRes, agentsRes]) => {
        console.log("Form data loaded:", { projRes, ttRes, agentsRes });
        setProject(projRes);
        console.log("Setting task types:", ttRes.task_types);
        setTaskTypes(ttRes.task_types);
        setAgents(agentsRes);
        
        // Set default agent
        const defaultAgent = projRes.default_agent || "claude-code";
        setAgentType(defaultAgent);
      })
      .catch((err) => {
        console.error("Failed to load form data:", err);
        setError("Failed to load project, templates or agents.");
      })
      .finally(() => setLoading(false));
  }, [resolvedParams.id]);

  const selectedType = taskTypes.find((t) => t.name === taskType);

  function handleTaskTypeSelect(name: string) {
    setTaskType(name);
    const tt = taskTypes.find((t) => t.name === name);
    if (tt) {
      const defaults: Record<string, string> = {};
      for (const input of tt.inputs) {
        defaults[input.name] = input.default ?? "";
      }
      setInputValues(defaults);
    }
  }

  function updateInput(name: string, value: string) {
    setInputValues((prev) => ({ ...prev, [name]: value }));
  }

  function getPrompt(): string {
    if (!selectedType) return "";
    const firstRequired = selectedType.inputs.find((i) => i.name === "prompt" || i.name === "requirement");
    if (firstRequired) {
      return inputValues[firstRequired.name] ?? "";
    }
    const anyRequired = selectedType.inputs.find((i) => i.required);
    return anyRequired ? (inputValues[anyRequired.name] ?? "") : "";
  }

  async function handleSubmit() {
    if (!project || !taskType || !title.trim()) return;

    setSubmitting(true);
    setError(null);

    try {
      const prompt = getPrompt();
      const finalInputs = {
        ...inputValues,
        agent_type: agentType,
        model: model || undefined,
      };

      const res = await createTask({
        title: title.trim(),
        prompt: prompt || title.trim(),
        project_id: project.id,
        task_type: taskType,
        inputs: JSON.stringify(finalInputs),
      });
      router.push(`/projects/${project.id}/tasks/${res.id}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create task");
      setSubmitting(false);
    }
  }

  if (loading) {
    return (
      <div className="space-y-6 max-w-4xl mx-auto">
        <Skeleton className="h-10 w-48" />
        <Card>
          <CardContent className="p-6 space-y-4">
            <Skeleton className="h-8 w-1/3" />
            <div className="grid grid-cols-2 gap-4">
              <Skeleton className="h-32" />
              <Skeleton className="h-32" />
            </div>
          </CardContent>
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

  const isStep1Valid = !!taskType;
  const isStep2Valid = !!title.trim() && (!selectedType || selectedType.inputs.filter(i => i.required).every(i => !!inputValues[i.name]?.trim()));

  return (
    <div className="max-w-4xl mx-auto space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" onClick={() => router.push(`/projects/${project.id}`)}>
          <IconArrowLeft className="h-5 w-5" />
        </Button>
        <h1 className="text-2xl font-bold tracking-tight">New Task</h1>
      </div>

      {/* Stepper */}
      <div className="flex items-center justify-between mb-8 px-4 sm:px-12 relative">
        <div className="absolute left-0 top-1/2 -translate-y-1/2 w-full h-0.5 bg-muted -z-10" />
        <div className="absolute left-0 top-1/2 -translate-y-1/2 h-0.5 bg-primary -z-10 transition-all duration-300" style={{ width: `${(step - 1) * 50}%` }} />
        
        {[
          { num: 1, label: "Select Template" },
          { num: 2, label: "Task Details" },
          { num: 3, label: "Confirm" }
        ].map((s) => (
          <div key={s.num} className="flex flex-col items-center bg-background px-2">
            <div className={cn(
              "w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium border-2 transition-colors",
              step > s.num ? "bg-primary border-primary text-primary-foreground" :
              step === s.num ? "border-primary text-primary" :
              "border-muted text-muted-foreground bg-background"
            )}>
              {step > s.num ? <IconCheck className="h-5 w-5" /> : s.num}
            </div>
            <span className={cn(
              "text-xs mt-2 font-medium absolute translate-y-10",
              step >= s.num ? "text-foreground" : "text-muted-foreground"
            )}>
              {s.label}
            </span>
          </div>
        ))}
      </div>

      <div className="mt-12">
        {error && (
          <div className="bg-destructive/10 border border-destructive/30 text-destructive rounded-md p-3 text-sm mb-6">
            {error}
          </div>
        )}

        <Card className="overflow-hidden">
          {/* STEP 1 */}
          {step === 1 && (
            <div className="animate-in fade-in slide-in-from-bottom-4 duration-300">
              <CardHeader>
                <CardTitle>Select Orchestration Template</CardTitle>
                <CardDescription>Choose how this task should be executed</CardDescription>
              </CardHeader>
              <CardContent className="grid grid-cols-1 md:grid-cols-2 gap-4">
                {taskTypes.map((tt) => (
                  <Card 
                    key={tt.name}
                    role="button"
                    tabIndex={0}
                    className={cn(
                      "cursor-pointer transition-all hover:border-primary/50 relative overflow-hidden",
                      taskType === tt.name ? "border-primary ring-1 ring-primary" : "border-border"
                    )}
                    onClick={() => handleTaskTypeSelect(tt.name)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        handleTaskTypeSelect(tt.name);
                      }
                    }}
                  >
                    <CardHeader className="pb-2">
                      <CardTitle className="text-lg flex items-center gap-2">
                        <IconBox className="h-5 w-5 text-primary" />
                        {tt.name}
                      </CardTitle>
                      <CardDescription>{tt.description}</CardDescription>
                    </CardHeader>
                    <CardContent>
                      <div className="flex items-center gap-2 text-xs text-muted-foreground bg-muted/50 p-2 rounded">
                        <IconPlayerPlay className="h-3 w-3" />
                        <span className="font-mono">
                          {tt.name === "single-stage" ? "[execute]" : "[coding] → [testing]"}
                        </span>
                      </div>
                    </CardContent>
                    {taskType === tt.name && (
                      <div className="absolute top-3 right-3 text-primary">
                        <IconCheck className="h-5 w-5" />
                      </div>
                    )}
                  </Card>
                ))}
              </CardContent>
              <CardFooter className="flex justify-end border-t p-4 bg-muted/20">
                <Button onClick={() => setStep(2)} disabled={!isStep1Valid}>
                  Next Step <IconChevronRight className="ml-2 h-4 w-4" />
                </Button>
              </CardFooter>
            </div>
          )}

          {/* STEP 2 */}
          {step === 2 && (
            <div className="animate-in fade-in slide-in-from-bottom-4 duration-300">
              <CardHeader>
                <CardTitle>Task Details</CardTitle>
                <CardDescription>Provide information for the {selectedType?.name} template</CardDescription>
              </CardHeader>
              <CardContent className="space-y-6">
                <div className="space-y-2">
                  <Label htmlFor="title">Task Title <span className="text-destructive">*</span></Label>
                  <Input
                    id="title"
                    value={title}
                    onChange={(e) => setTitle(e.target.value)}
                    placeholder="e.g., Implement user authentication"
                    autoFocus
                  />
                </div>

                {selectedType?.inputs.map((input) => (
                  <div key={input.name} className="space-y-2">
                    <Label htmlFor={input.name}>
                      {input.name === "prompt" || input.name === "requirement" ? "Requirement" : input.name}
                      {input.required && <span className="text-destructive ml-1">*</span>}
                    </Label>
                    
                    {input.name === "prompt" || input.name === "requirement" ? (
                      <Textarea
                        id={input.name}
                        value={inputValues[input.name] ?? ""}
                        onChange={(e) => updateInput(input.name, e.target.value)}
                        placeholder="Describe what needs to be done..."
                        rows={6}
                        className="font-mono text-sm"
                      />
                    ) : (
                      <Input
                        id={input.name}
                        value={inputValues[input.name] ?? ""}
                        onChange={(e) => updateInput(input.name, e.target.value)}
                        placeholder={input.default ?? ""}
                      />
                    )}
                  </div>
                ))}

                <div className="pt-4 border-t">
                  <Button 
                    variant="ghost" 
                    className="w-full justify-between p-0 h-auto hover:bg-transparent"
                    onClick={() => setShowAdvanced(!showAdvanced)}
                  >
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium">Advanced Options</span>
                    </div>
                    <IconChevronRight className={cn("h-4 w-4 transition-transform", showAdvanced && "rotate-90")} />
                  </Button>
                  
                  {showAdvanced && (
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mt-4 animate-in fade-in slide-in-from-top-2">
                      <div className="space-y-2">
                        <Label htmlFor="agentType" className="text-xs flex items-center gap-2">
                          <IconRobot className="h-3 w-3" /> Agent Type
                        </Label>
                        <Select value={agentType} onValueChange={setAgentType}>
                          <SelectTrigger className="text-sm h-9">
                            <SelectValue placeholder="Select Agent" />
                          </SelectTrigger>
                          <SelectContent>
                            {agents
                              .filter(a => a.installed)
                              .map(a => (
                                <SelectItem key={a.type} value={a.type}>
                                  {a.name}
                                </SelectItem>
                              ))
                            }
                          </SelectContent>
                        </Select>
                      </div>
                      <div className="space-y-2">
                        <Label htmlFor="model" className="text-xs">Model Override</Label>
                        <Input
                          id="model"
                          value={model}
                          onChange={(e) => setModel(e.target.value)}
                          placeholder="Leave empty for default (e.g. sonnet, opus, o3)"
                          className="text-sm h-9"
                        />
                      </div>
                    </div>
                  )}
                </div>
              </CardContent>
              <CardFooter className="flex justify-between border-t p-4 bg-muted/20">
                <Button variant="outline" onClick={() => setStep(1)}>
                  Back
                </Button>
                <Button onClick={() => setStep(3)} disabled={!isStep2Valid}>
                  Review <IconChevronRight className="ml-2 h-4 w-4" />
                </Button>
              </CardFooter>
            </div>
          )}

          {/* STEP 3 */}
          {step === 3 && (
            <div className="animate-in fade-in slide-in-from-bottom-4 duration-300">
              <CardHeader>
                <CardTitle>Confirm Task</CardTitle>
                <CardDescription>Review details before creating the task</CardDescription>
              </CardHeader>
              <CardContent className="space-y-6">
                <div className="grid grid-cols-2 gap-y-4 text-sm">
                  <div className="text-muted-foreground">Project</div>
                  <div className="font-medium">{project.name}</div>
                  
                  <div className="text-muted-foreground">Template</div>
                  <div className="font-medium font-mono">{taskType}</div>
                  
                  <div className="text-muted-foreground">Title</div>
                  <div className="font-medium">{title}</div>
                  
                  <div className="text-muted-foreground">Agent</div>
                  <div className="font-medium font-mono">
                    {agents.find(a => a.type === agentType)?.name || agentType}
                  </div>
                  
                  {model && (
                    <>
                      <div className="text-muted-foreground">Model</div>
                      <div className="font-medium font-mono">{model}</div>
                    </>
                  )}
                </div>

                <div className="border rounded-md p-4 bg-muted/30">
                  <div className="text-xs text-muted-foreground mb-2 font-semibold tracking-wider">Requirement</div>
                  <div className="font-mono text-sm whitespace-pre-wrap max-h-60 overflow-y-auto">
                    {getPrompt()}
                  </div>
                </div>
              </CardContent>
              <CardFooter className="flex justify-between border-t p-4 bg-muted/20">
                <Button variant="outline" onClick={() => setStep(2)} disabled={submitting}>
                  Back
                </Button>
                <Button onClick={handleSubmit} disabled={submitting}>
                  {submitting ? "Creating Task..." : "Create Task"}
                </Button>
              </CardFooter>
            </div>
          )}
        </Card>
      </div>
    </div>
  );
}