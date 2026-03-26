import type { Metadata } from "next";
import Link from "next/link";
import "./globals.css";

export const metadata: Metadata = {
  title: "CCodeBoX",
  description: "Task-driven code automation platform",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="dark">
      <body className="min-h-screen">
        <header className="border-b border-border sticky top-0 z-50 bg-bg-surface/80 backdrop-blur-sm">
          <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
            <div className="flex items-center justify-between h-14">
              <Link
                href="/"
                className="text-lg font-bold tracking-tight text-text-primary hover:text-primary transition-colors"
              >
                CCodeBoX
              </Link>
              <nav className="flex items-center gap-4">
                <Link
                  href="/"
                  className="text-sm text-text-secondary hover:text-text-primary transition-colors"
                >
                  Tasks
                </Link>
                <Link
                  href="/settings"
                  className="text-sm text-text-secondary hover:text-text-primary transition-colors"
                >
                  Settings
                </Link>
                <Link
                  href="/tasks/new"
                  className="text-sm bg-primary hover:bg-primary-hover text-white px-3 py-1.5 rounded-md transition-colors"
                >
                  New Task
                </Link>
              </nav>
            </div>
          </div>
        </header>
        <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
          {children}
        </main>
      </body>
    </html>
  );
}
