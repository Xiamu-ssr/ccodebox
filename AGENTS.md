# AGENTS.md — CCodeBoX

## 项目简介

CCodeBoX 是一个任务驱动的代码自动化平台，在容器中编排 Coding Agent（Claude Code / Codex CLI），实现 **写代码 → 测试 → 修复 → 交付** 的自主闭环。

## 技术栈

- **后端**: Rust (axum + bollard + sqlx + SQLite)
- **前端**: Next.js 14+ (App Router, TypeScript, Tailwind CSS)
- **容器**: Podman/Docker (通过 bollard crate 的 Docker API)
- **Agent 镜像**: 预构建的 Docker 镜像，内含 Claude Code 或 Codex CLI

## 项目结构

```
ccodebox/
├── AGENTS.md
├── README.md
├── backend/                 # Rust 后端
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs          # 入口 + axum router
│   │   ├── api/             # HTTP handlers
│   │   │   ├── mod.rs
│   │   │   └── tasks.rs     # 任务 CRUD
│   │   ├── container/       # 容器编排
│   │   │   ├── mod.rs
│   │   │   └── manager.rs   # bollard 封装
│   │   ├── db/              # 数据库
│   │   │   ├── mod.rs
│   │   │   └── migrations/  # SQL 迁移文件
│   │   └── models/          # 数据模型
│   │       ├── mod.rs
│   │       └── task.rs
│   └── .env.example
├── frontend/                # Next.js 前端
│   ├── package.json
│   ├── next.config.js
│   ├── tailwind.config.ts
│   ├── tsconfig.json
│   └── src/
│       ├── app/
│       │   ├── layout.tsx
│       │   ├── page.tsx           # 任务列表
│       │   └── tasks/
│       │       ├── new/page.tsx   # 创建任务
│       │       └── [id]/page.tsx  # 任务详情
│       ├── components/
│       │   ├── TaskCard.tsx
│       │   ├── TaskForm.tsx
│       │   ├── TaskDetail.tsx
│       │   ├── LogViewer.tsx
│       │   └── StatusBadge.tsx
│       └── lib/
│           ├── api.ts             # 后端 API client
│           └── types.ts           # TypeScript 类型
├── images/                  # 容器镜像定义
│   ├── base/
│   │   └── Dockerfile       # 基础镜像 (node + python + tools)
│   └── claude-code/
│       └── Dockerfile       # CC agent 镜像 (base + claude-code CLI)
├── scripts/
│   └── entrypoint.sh        # 容器内 Loop 逻辑
└── docs/
    └── design.md            # 设计文档
```

## 构建与运行

```bash
# 后端
cd backend
cargo build
cargo run  # 默认 :3000

# 前端
cd frontend
npm install
npm run dev  # 默认 :3001

# 构建 agent 镜像
docker build -t ccodebox-base:latest -f images/base/Dockerfile images/base/
docker build -t ccodebox-cc:latest -f images/claude-code/Dockerfile .
```

## 代码规范

### Rust
- 用 `cargo clippy` 检查，0 warning
- 用 `cargo fmt` 格式化
- 错误处理用 `anyhow::Result` (应用层) 和 `thiserror` (库层)
- 异步用 tokio，不用 block_on

### TypeScript
- 用 ESLint + Prettier
- 严格模式 (`strict: true`)
- 组件用函数式 + hooks
- API 调用统一走 `lib/api.ts`

### 通用
- commit message 格式: `type(scope): description` (feat/fix/refactor/docs/chore)
- 每个 PR 一个功能点
- 不留 TODO 注释，要做就做，不做就删

## 实施约束（重要！）

1. **不做向后兼容** — 这是全新项目，没有旧代码需要兼容
2. **不加多余抽象** — 先跑通，再优化。不提前写 trait/interface "以备将来"
3. **每步都验证** — 改完 Rust 跑 `cargo check && cargo clippy`，改完前端跑 `npm run build`
4. **删掉不用的代码** — 不留注释掉的代码块
5. **一次只做一件事** — 按 Exec Plan 的步骤顺序执行，不跳步

# 开发规范
## TDD开发流程
1. 先写测试（按功能模块粒度，不是单函数），跑 `cargo test` 确认编译通过但断言失败
2. 实现代码，跑 `cargo test` 确认绿灯
3. 需要时重构，保持绿灯
4. 测试必须覆盖实际代码路径，不是理想调用方式

## 隐式契约防御

**必读**：`.claude/skills/implicit-contract-defense/SKILL.md`

所有跨边界交互（前后端、数据库、外部输入）必须收敛到隔离仓。开发前先读 skill，每次改完跑 `check_contracts.sh` 验证。
