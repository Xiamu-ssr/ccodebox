"use client";

import ReactMarkdown from "react-markdown";

export default function MarkdownViewer({
  content,
}: {
  content: string | null;
}) {
  if (!content || content.trim() === "") {
    return (
      <div className="text-center py-10 text-text-secondary text-sm">
        No summary available.
      </div>
    );
  }

  return (
    <div className="prose prose-invert prose-sm max-w-none bg-bg-surface border border-border rounded-md p-6">
      <ReactMarkdown
        components={{
          h1: ({ children }) => (
            <h1 className="text-lg font-bold text-text-primary border-b border-border pb-2 mb-4">
              {children}
            </h1>
          ),
          h2: ({ children }) => (
            <h2 className="text-base font-semibold text-text-primary mt-6 mb-3">
              {children}
            </h2>
          ),
          h3: ({ children }) => (
            <h3 className="text-sm font-semibold text-text-primary mt-4 mb-2">
              {children}
            </h3>
          ),
          p: ({ children }) => (
            <p className="text-sm text-text-secondary leading-relaxed mb-3">
              {children}
            </p>
          ),
          code: ({ children, className }) => {
            const isInline = !className;
            if (isInline) {
              return (
                <code className="bg-bg-base px-1.5 py-0.5 rounded text-xs font-mono text-primary">
                  {children}
                </code>
              );
            }
            return (
              <code className="block bg-bg-base rounded-md p-3 text-xs font-mono text-text-primary overflow-auto">
                {children}
              </code>
            );
          },
          ul: ({ children }) => (
            <ul className="list-disc list-inside space-y-1 text-sm text-text-secondary">
              {children}
            </ul>
          ),
          ol: ({ children }) => (
            <ol className="list-decimal list-inside space-y-1 text-sm text-text-secondary">
              {children}
            </ol>
          ),
          li: ({ children }) => <li className="leading-relaxed">{children}</li>,
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
