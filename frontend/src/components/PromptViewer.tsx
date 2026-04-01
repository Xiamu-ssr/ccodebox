"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { IconChevronDown, IconChevronUp } from "@tabler/icons-react";

interface PromptViewerProps {
  prompt: string | null;
}

export default function PromptViewer({ prompt }: PromptViewerProps) {
  const [showFull, setShowFull] = useState(false);
  const [expandedSections, setExpandedSections] = useState<Record<number, boolean>>({
    2: true // Default expand the last section (Task)
  });

  if (!prompt) {
    return <div className="text-muted-foreground text-sm py-4">No prompt recorded</div>;
  }

  const sections = prompt.split("\n---\n").map(s => s.trim());
  const sectionTitles = ["Platform Rules", "Project Rules (AGENTS.md)", "Task Requirement"];

  if (showFull || sections.length < 2) {
    return (
      <div className="space-y-4">
        {sections.length >= 2 && (
          <Button variant="outline" size="sm" onClick={() => setShowFull(false)}>
            Show Sections
          </Button>
        )}
        <div className="bg-muted p-4 rounded-md border">
          <pre className="text-xs font-mono whitespace-pre-wrap break-words">
            {prompt}
          </pre>
        </div>
      </div>
    );
  }

  const toggleSection = (idx: number) => {
    setExpandedSections(prev => ({ ...prev, [idx]: !prev[idx] }));
  };

  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <Button variant="ghost" size="xs" className="h-7 text-xs" onClick={() => setShowFull(true)}>
          Show Full Prompt
        </Button>
      </div>
      
      {sections.map((content, i) => {
        const title = sectionTitles[i] || `Section ${i + 1}`;
        const isExpanded = expandedSections[i];
        
        return (
          <div key={i} className="border rounded-md overflow-hidden">
            <button 
              type="button"
              className="w-full flex items-center justify-between px-3 py-2 bg-muted/50 hover:bg-muted transition-colors text-left"
              onClick={() => toggleSection(i)}
            >
              <span className="text-xs font-semibold">{title}</span>
              {isExpanded ? <IconChevronUp className="h-4 w-4 text-muted-foreground" /> : <IconChevronDown className="h-4 w-4 text-muted-foreground" />}
            </button>
            {isExpanded && (
              <div className="p-3 bg-background border-t">
                <pre className="text-[11px] font-mono whitespace-pre-wrap break-words max-h-[300px] overflow-y-auto">
                  {content}
                </pre>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
