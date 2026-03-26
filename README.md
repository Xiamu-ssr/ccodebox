# CCodeBoX 📦

Task-driven code automation platform. Orchestrates Coding Agents (Claude Code / Codex CLI) in containers to achieve autonomous **write → test → fix → deliver** loops.

## Quick Start

```bash
# 1. Build agent images
docker build -t ccodebox-base:latest -f images/base/Dockerfile images/base/
docker build -t ccodebox-cc:latest -f images/claude-code/Dockerfile .

# 2. Start backend
cd backend
cp .env.example .env  # Edit with your API keys
cargo run

# 3. Start frontend
cd frontend
npm install
npm run dev

# 4. Open http://localhost:3001
```

## Architecture

```
┌──────────────┐     ┌───────────────┐     ┌─────────────────────┐
│   Frontend   │────▶│   Backend     │────▶│  Container (Agent)  │
│  (Next.js)   │◀────│  (Rust/axum)  │◀────│  CC / Codex CLI     │
│  :3001       │     │  :3000        │     │  entrypoint.sh loop │
└──────────────┘     └───────┬───────┘     └─────────────────────┘
                             │
                       ┌─────┴─────┐
                       │  SQLite   │
                       └───────────┘
```

## How the Loop Works

1. User submits a task (prompt + optional repo)
2. Backend creates a container with the chosen agent
3. Inside the container, `entrypoint.sh` runs:
   - Agent receives prompt and writes code
   - Wrapper independently runs lint + tests
   - If failed → errors fed back to agent for next round
   - Repeat up to max_rounds
4. Container exits, backend collects `report.json` and artifacts
5. Frontend displays results (status, diff, logs, summary)

## License

MIT
