"use client";

import ReactDiffViewer, { DiffMethod } from "react-diff-viewer-continued";

export default function DiffViewer({ diff }: { diff: string | null }) {
  if (!diff || diff.trim() === "") {
    return (
      <div className="text-center py-10 text-muted-foreground text-sm">
        No diff available.
      </div>
    );
  }

  // Split by files if multiple
  const files = diff.split(/^diff --git /m).filter(f => f.trim() !== "");

  return (
    <div className="space-y-6">
      {files.map((fileDiff, idx) => {
        // Simple extraction of file name
        const lines = fileDiff.split("\n");
        const fileNameLine = lines.find(l => l.startsWith("--- a/") || l.startsWith("+++ b/"));
        const fileName = fileNameLine ? fileNameLine.substring(6) : `File ${idx + 1}`;
        
        // Reconstruct old and new values for the file
        let oldValue = "";
        let newValue = "";
        
        let inHunk = false;
        for (const line of lines) {
          if (line.startsWith("@@")) {
            inHunk = true;
            continue;
          }
          if (!inHunk) continue;
          
          if (line.startsWith("-")) {
            oldValue += line.substring(1) + "\n";
          } else if (line.startsWith("+")) {
            newValue += line.substring(1) + "\n";
          } else if (line.startsWith(" ")) {
            oldValue += line.substring(1) + "\n";
            newValue += line.substring(1) + "\n";
          } else if (line.startsWith("\\ No newline at end of file")) {
            // skip
          } else {
            // maybe next hunk or next file?
            // in git diff, everything after hunks is next file or done
          }
        }

        return (
          <div key={idx} className="border rounded-lg overflow-hidden">
            <div className="bg-muted px-4 py-2 text-xs font-mono border-b flex items-center justify-between">
              <span className="truncate">{fileName}</span>
            </div>
            <div className="overflow-x-auto">
              <ReactDiffViewer
                oldValue={oldValue}
                newValue={newValue}
                splitView={false}
                useDarkTheme={true}
                styles={{
                  variables: {
                    dark: {
                      diffViewerBackground: "#09090b",
                      diffViewerTitleBackground: "#18181b",
                      diffViewerTitleColor: "#71717a",
                      removedBackground: "#450a0a",
                      removedColor: "#f87171",
                      addedBackground: "#064e3b",
                      addedColor: "#4ade80",
                      wordRemovedBackground: "#7f1d1d",
                      wordAddedBackground: "#065f46",
                      codeFoldGutterBackground: "#18181b",
                      codeFoldBackground: "#09090b",
                      codeFoldContentColor: "#71717a",
                    }
                  },
                  contentText: {
                    fontSize: "12px",
                    fontFamily: "var(--font-geist-mono), ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace",
                  }
                }}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}
