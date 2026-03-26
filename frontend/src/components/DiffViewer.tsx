"use client";

export default function DiffViewer({ diff }: { diff: string | null }) {
  if (!diff || diff.trim() === "") {
    return (
      <div className="text-center py-10 text-text-secondary text-sm">
        No diff available.
      </div>
    );
  }

  const lines = diff.split("\n");

  return (
    <pre className="bg-bg-base border border-border rounded-md p-4 text-xs font-mono overflow-auto max-h-[600px]">
      {lines.map((line, i) => {
        let className = "text-text-primary";
        if (line.startsWith("+") && !line.startsWith("+++")) {
          className = "text-status-success bg-status-success/10";
        } else if (line.startsWith("-") && !line.startsWith("---")) {
          className = "text-status-failed bg-status-failed/10";
        } else if (line.startsWith("@@")) {
          className = "text-primary bg-primary/10";
        } else if (line.startsWith("diff ") || line.startsWith("index ")) {
          className = "text-text-muted font-bold";
        }

        return (
          <div key={i} className={`${className} px-2 leading-5`}>
            {line || " "}
          </div>
        );
      })}
    </pre>
  );
}
