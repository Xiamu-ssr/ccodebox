"use client";

import { useState } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { IconRobot, IconTool, IconCheck, IconEye, IconEyeOff } from "@tabler/icons-react";
import { cn } from "@/lib/utils";

interface AgentLogViewerProps {
  log: string | null;
  status: string;
}

export default function AgentLogViewer({ log, status }: AgentLogViewerProps) {
  const [showRaw, setShowRaw] = useState(false);

  if (!log) {
    return (
      <div className="bg-zinc-950 text-zinc-500 h-[400px] flex items-center justify-center rounded-md border border-zinc-800 text-sm">
        {status === "running" ? "Waiting for logs..." : "No logs available"}
      </div>
    );
  }

  // Try to parse as JSON lines
  const lines = log.trim().split("\n");
  let parsedLines: any[] = [];
  let isJson = true;

  try {
    parsedLines = lines.map(line => JSON.parse(line));
  } catch (e) {
    isJson = false;
  }

  if (!isJson || showRaw) {
    return (
      <div className="bg-zinc-950 text-zinc-50 p-4 rounded-md border border-zinc-800 relative group">
        <Button 
          variant="ghost" 
          size="sm" 
          className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity bg-zinc-900/50 hover:bg-zinc-800 text-zinc-400"
          onClick={() => setShowRaw(!showRaw)}
        >
          {showRaw ? <IconEyeOff className="h-4 w-4 mr-2" /> : <IconEye className="h-4 w-4 mr-2" />}
          {showRaw ? "Formatted" : "Raw"}
        </Button>
        <ScrollArea className="h-[400px] w-full">
          <pre className="text-xs font-mono whitespace-pre-wrap break-words pb-4">
            {log}
          </pre>
        </ScrollArea>
      </div>
    );
  }

  return (
    <div className="bg-zinc-950 text-zinc-50 p-4 rounded-md border border-zinc-800 relative group">
      <Button 
        variant="ghost" 
        size="sm" 
        className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity bg-zinc-900/50 hover:bg-zinc-800 text-zinc-400 z-20"
        onClick={() => setShowRaw(!showRaw)}
      >
        <IconEye className="h-4 w-4 mr-2" />
        Raw
      </Button>
      <ScrollArea className="h-[400px] w-full pr-4">
        <div className="space-y-4 pb-4">
          {parsedLines.map((data, i) => {
            if (data.type === "assistant") {
              return (
                <div key={i} className="space-y-1">
                  <div className="text-[10px] text-zinc-500 uppercase font-bold flex items-center gap-1.5">
                    <IconRobot className="h-3 w-3" /> Assistant
                  </div>
                  <div className="text-sm text-zinc-200 whitespace-pre-wrap">
                    {data.message?.content || data.content || JSON.stringify(data)}
                  </div>
                </div>
              );
            }
            if (data.type === "tool_use") {
              return (
                <div key={i} className="bg-zinc-900/50 border border-zinc-800 rounded p-2.5 space-y-2">
                  <div className="text-[10px] text-zinc-400 uppercase font-bold flex items-center gap-1.5">
                    <IconTool className="h-3 w-3" /> Tool Call: <span className="text-blue-400 lowercase font-mono">{data.name}</span>
                  </div>
                  {data.input && (
                    <pre className="text-[10px] font-mono text-zinc-500 bg-black/30 p-1.5 rounded overflow-x-auto">
                      {typeof data.input === 'string' ? data.input : JSON.stringify(data.input, null, 2)}
                    </pre>
                  )}
                </div>
              );
            }
            if (data.type === "result") {
              return (
                <div key={i} className="bg-green-500/10 border border-green-500/30 rounded p-3 space-y-2">
                  <div className="text-[10px] text-green-500 uppercase font-bold flex items-center gap-1.5">
                    <IconCheck className="h-3 w-3" /> Result
                  </div>
                  <div className="text-sm text-green-200 font-medium">
                    {typeof data.content === 'string' ? data.content : JSON.stringify(data.content)}
                  </div>
                </div>
              );
            }
            // Other types: system, tool_output, etc.
            return (
              <div key={i} className="text-[10px] font-mono text-zinc-600 border-l-2 border-zinc-800 pl-2">
                [{data.type}] {JSON.stringify(data)}
              </div>
            );
          })}
        </div>
      </ScrollArea>
    </div>
  );
}
