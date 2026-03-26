# Phase 2 — 打包、部署与配置补全

> 目标：一个二进制、一条命令启动、GitHub Release 自动发布。
> 本文覆盖 Phase 1 遗留修复 + 单二进制打包 + CI/CD。
> 与 Phase 1 设计文档冲突时以本文为准。

---

## 一、Phase 1 遗留修复

### 1.1 Codex 补全 OPENAI_BASE_URL

**问题**：phase1-env-design.md 要求 Codex 支持 `OPENAI_BASE_URL`（第三方供应商），但 manager.rs 和前端都未实现。

**后端 container/manager.rs**：

```rust
AgentType::Codex => {
    env.push(format!("CODEX_MODEL={}", task.model));
    if let Some(v) = env_config.get("agent.codex.api_key") {
        env.push(format!("OPENAI_API_KEY={v}"));
    }
    // ↓ 新增
    if let Some(v) = env_config.get("agent.codex.api_base_url") {
        env.push(format!("OPENAI_BASE_URL={v}"));
    }
}
```

**前端 settings/page.tsx**：

```typescript
// 修改 AGENT_CONFIG
codex: { keyPrefix: "agent.codex", hasBaseUrl: true },
```

### 1.2 模型选择改为自由输入

**问题**：硬编码模型列表不适配第三方供应商。

**删除**：config.rs 中 `AgentInfo.models` 的硬编码列表，改为空数组或从 API 动态获取。

**前端 TaskForm.tsx 改动**：

- Model 字段从 `<select>` 改为 `<input type="text">`，附带 `<datalist>` 提供建议
- 默认值填 settings 返回的 `default_model`
- 建议列表来源：settings 中的 `models`（如果有）或用户历史输入

**后端新增可选接口**（低优先级，不阻塞主线）：

```
POST /api/settings/list-models
Body: { "agent_type": "claude_code" }
Response: { "models": ["claude-sonnet-4-20250514", "claude-opus-4-6", ...] }
```

实现：根据 agent_type 读 DB 中的 api_base_url + api_key，调 `{base_url}/v1/models`，返回模型列表。失败则返回空列表（不报错）。

### 1.3 清理 .env.example

替换为仅包含代码实际读取的环境变量：

```env
# Server
CCODEBOX_HOST=0.0.0.0
CCODEBOX_PORT=3000
DATABASE_URL=sqlite:./data/ccodebox.db

# Container images (override defaults)
# CC_IMAGE=ccodebox-cc:latest
# CODEX_IMAGE=ccodebox-codex:latest

# Default model for new tasks
# CC_DEFAULT_MODEL=claude-sonnet-4-20250514

# Container resource limits
# CONTAINER_MEMORY_LIMIT=4294967296  # 4GB
# CONTAINER_CPU_QUOTA=200000          # 2 cores

# Logging
RUST_LOG=info
```

注释说明：API Key 等敏感配置在 Web UI 的 Settings 页面管理，不在 .env 中。

---

## 二、前后端单二进制打包

### 2.1 架构

```
构建时：
  npm run build (next export) → frontend/out/ (纯静态 HTML/JS/CSS)
  cargo build --release (rust-embed 嵌入 out/) → ccodebox 单二进制

运行时：
  ./ccodebox
    ├── GET /api/*     → axum API handlers
    ├── GET /静态文件   → 从内嵌的 out/ 返回
    └── GET /其他路径   → 返回 index.html（SPA fallback）
```

### 2.2 前端改动

**next.config.ts**：

```typescript
import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "export",
  // 删除 rewrites — 前后端同源，不需要代理
};

export default nextConfig;
```

**api.ts** — API_BASE 保持 `/api` 不变，同源请求无需代理。

**tasks/[id]/page.tsx** — 添加空的 `generateStaticParams` 以兼容静态导出：

```typescript
export function generateStaticParams() {
  return [];
}
```

动态路由由 SPA fallback 处理（见 2.3）。

### 2.3 后端改动

**Cargo.toml 新增依赖**：

```toml
rust-embed = { version = "8", features = ["compression"] }
mime_guess = "2"
```

**新增 frontend.rs 模块**：

```rust
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../frontend/out"]
struct Assets;

pub async fn serve_frontend(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // 1. 精确匹配静态文件
    if let Some(file) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_ref())],
            file.data,
        )
            .into_response();
    }

    // 2. 尝试 path/index.html（Next.js export 目录结构）
    let index_path = if path.is_empty() {
        "index.html".to_string()
    } else {
        format!("{path}/index.html")
    };
    if let Some(file) = Assets::get(&index_path) {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html")],
            file.data,
        )
            .into_response();
    }

    // 3. SPA fallback — 返回根 index.html，客户端路由接管
    match Assets::get("index.html") {
        Some(file) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html")],
            file.data,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "Frontend not found").into_response(),
    }
}
```

**api/mod.rs 修改路由**：

```rust
pub fn router<R: ContainerRuntime>(state: Arc<AppState<R>>) -> Router {
    Router::new()
        // API 路由（优先匹配）
        .route("/api/tasks", post(tasks::create_task::<R>))
        .route("/api/tasks", get(tasks::list_tasks::<R>))
        // ... 其他 API 路由 ...
        .with_state(state)
        // 前端静态文件 + SPA fallback（兜底）
        .fallback(crate::frontend::serve_frontend)
}
```

**端口合并**：只暴露一个端口（默认 3000），前后端同源。

### 2.4 路由匹配顺序

```
请求 GET /api/tasks       → axum route → API handler
请求 GET /settings         → fallback → Assets::get("settings/index.html") → 静态 HTML
请求 GET /tasks/abc123     → fallback → Assets::get("tasks/abc123") 无 → SPA fallback → index.html → 客户端路由
请求 GET /_next/static/... → fallback → Assets::get("_next/static/...") → JS/CSS 文件
请求 GET /favicon.ico      → fallback → Assets::get("favicon.ico") → 静态文件
```

---

## 三、Docker 镜像首次自动构建

### 3.1 问题

用户首次启动 ccodebox，本地没有 `ccodebox-base:latest` / `ccodebox-cc:latest` / `ccodebox-codex:latest`，提交任务会报 404。

### 3.2 方案：Dockerfile 嵌入二进制 + 按需构建

**构建时**：用 `include_str!` 将 Dockerfile + 辅助文件嵌入二进制。

```rust
const BASE_DOCKERFILE: &str = include_str!("../../images/base/Dockerfile");
const CC_DOCKERFILE: &str = include_str!("../../images/claude-code/Dockerfile");
const CODEX_DOCKERFILE: &str = include_str!("../../images/codex/Dockerfile");
const SYSTEM_RULES: &str = include_str!("../../scripts/system-rules.md");
const ENTRYPOINT_SH: &str = include_str!("../../scripts/entrypoint.sh");
const TAVILY_SEARCH: &str = include_str!("../../scripts/tavily-search");
```

**运行时**：启动时检查镜像是否存在，不存在则自动构建。

```rust
// container/images.rs
pub async fn ensure_images(docker: &Docker, config: &PlatformConfig) -> Result<()> {
    let required = [
        ("ccodebox-base:latest", BuildSpec::Base),
        (&config.cc_image, BuildSpec::ClaudeCode),
        (&config.codex_image, BuildSpec::Codex),
    ];

    for (image, spec) in &required {
        if !image_exists(docker, image).await? {
            tracing::info!("Building {image}...");
            build_image(docker, image, spec).await?;
        }
    }
    Ok(())
}
```

构建流程：
1. 创建临时目录
2. 将嵌入的 Dockerfile + scripts 写出到临时目录
3. 调用 bollard 的 build image API
4. 构建完成后删除临时目录

### 3.3 前端状态展示

Settings 页面新增「镜像状态」区域：

```
Agent Images
┌──────────────────────────────────────────┐
│ ccodebox-base:latest     ✅ Ready        │
│ ccodebox-cc:latest       ✅ Ready        │
│ ccodebox-codex:latest    ❌ Not Built    │
│                          [Build Now]     │
└──────────────────────────────────────────┘
```

**新增 API**：

```
GET  /api/settings/images       → 返回各镜像状态
POST /api/settings/images/build → 触发构建（异步，返回 202）
```

### 3.4 CLI 子命令

```bash
ccodebox setup    # 手动触发镜像构建
ccodebox serve    # 启动服务（默认行为，等同于不带子命令）
ccodebox version  # 版本信息
```

子命令解析用 `clap`（或手动解析 args，保持依赖精简）。

---

## 四、GitHub Actions CI/CD

### 4.1 构建矩阵

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ["v*"]

jobs:
  build:
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
            artifact: ccodebox-linux-amd64
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest
            artifact: ccodebox-linux-arm64
          - target: x86_64-apple-darwin
            os: macos-latest
            artifact: ccodebox-macos-amd64
          - target: aarch64-apple-darwin
            os: macos-latest
            artifact: ccodebox-macos-arm64

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4

      # 1. 构建前端
      - uses: actions/setup-node@v4
        with:
          node-version: "22"
      - name: Build frontend
        working-directory: frontend
        run: |
          npm ci
          npm run build

      # 2. 构建后端（rust-embed 自动嵌入 frontend/out/）
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - name: Install cross-compilation tools
        if: matrix.target == 'aarch64-unknown-linux-gnu'
        run: |
          sudo apt-get update
          sudo apt-get install -y gcc-aarch64-linux-gnu
          echo "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc" >> $GITHUB_ENV
      - name: Build backend
        working-directory: backend
        run: cargo build --release --target ${{ matrix.target }}

      # 3. 打包
      - name: Package
        run: |
          mkdir -p dist
          cp backend/target/${{ matrix.target }}/release/ccodebox-backend dist/${{ matrix.artifact }}
          chmod +x dist/${{ matrix.artifact }}

      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.artifact }}
          path: dist/${{ matrix.artifact }}

  release:
    needs: build
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/download-artifact@v4
        with:
          path: artifacts
          merge-multiple: true

      - uses: softprops/action-gh-release@v2
        with:
          files: artifacts/*
          generate_release_notes: true
```

### 4.2 产物

每个 Release 包含 4 个二进制：

| 文件名 | 平台 |
|--------|------|
| ccodebox-linux-amd64 | Linux x86_64 |
| ccodebox-linux-arm64 | Linux ARM64 (树莓派/云服务器) |
| ccodebox-macos-amd64 | macOS Intel |
| ccodebox-macos-arm64 | macOS Apple Silicon |

### 4.3 用户安装流程

```bash
# macOS Apple Silicon
curl -fsSL https://github.com/{owner}/ccodebox/releases/latest/download/ccodebox-macos-arm64 -o ccodebox
chmod +x ccodebox
./ccodebox
# 首次启动自动构建 Docker 镜像
# 打开 http://localhost:3000 配置 API Key，开始使用
```

---

## 五、实施约束（CC 必读）

1. **不做向后兼容**——直接改，不留旧代码
2. **删掉不用的代码**——config.rs 中的 `models` 硬编码列表、前端 `<select>` 等
3. **每步跑 `scripts/check_contracts.sh`** + `cargo test` + `cd frontend && npm run build`
4. **二进制名**：`Cargo.toml` 的 `[[bin]]` name 改为 `ccodebox`（当前是 `ccodebox-backend`）
5. **前端 build 目录**：rust-embed 的 `#[folder]` 路径是 `../frontend/out`，确保 CI 中先 build 前端再 build 后端
6. **CORS**：前后端同源后，CorsLayer 可以收紧或移除（开发时保留 `Any` 也可以）

## 六、CC 执行顺序

### Phase A：遗留修复（先做）

1. manager.rs — Codex 分支新增 `OPENAI_BASE_URL` 注入
2. 前端 settings/page.tsx — Codex 卡片 `hasBaseUrl` 改 `true`
3. 前端 TaskForm.tsx — Model 从 `<select>` 改为 `<input>` + `<datalist>`
4. config.rs — `AgentInfo.models` 改为空 vec（或删除 models 字段）
5. contracts.rs — 如果删 models 字段，同步更新 AgentInfo 类型
6. .env.example — 替换为只含实际读取的变量
7. 运行 `scripts/check_contracts.sh` + `cargo test` + `npm run build`

### Phase B：单二进制打包

8. `next.config.ts` — 改为 `output: 'export'`，删除 `rewrites`
9. `tasks/[id]/page.tsx` — 添加 `generateStaticParams`
10. 验证 `npm run build` 产出 `frontend/out/` 目录
11. `Cargo.toml` — 添加 `rust-embed` + `mime_guess`，`[[bin]]` name 改 `ccodebox`
12. 新建 `backend/src/frontend.rs` — serve_frontend 逻辑
13. `api/mod.rs` — 添加 `.fallback(frontend::serve_frontend)`
14. `main.rs` — 引入 `mod frontend`
15. 验证 `cargo build --release` 后 `./ccodebox` 单端口同时提供 API + 前端
16. 运行 `cargo test` 确认 API 测试不受影响

### Phase C：镜像自动构建

17. 新建 `backend/src/container/images.rs` — 嵌入 Dockerfile + ensure_images
18. `main.rs` — 启动时调用 `ensure_images`
19. `api/mod.rs` — 新增 `/api/settings/images` + `/api/settings/images/build`
20. 前端 settings 页面新增镜像状态区域
21. `main.rs` — 添加 CLI 子命令解析（setup / serve / version）

### Phase D：CI/CD

22. 新建 `.github/workflows/release.yml`
23. 本地验证 `npm run build && cargo build --release` 产出可用二进制
24. 打 tag 触发 CI，确认 Release 页面有 4 个二进制产物

---

## 七、验收标准

- [ ] `./ccodebox` 单命令启动，`http://localhost:3000` 同时提供 UI 和 API
- [ ] `/tasks/任意ID` 能正确渲染（SPA fallback 生效）
- [ ] Settings 页面可配置 Codex 的 Base URL
- [ ] Model 字段为自由输入文本框
- [ ] 首次启动自动构建缺失的 Docker 镜像
- [ ] `git tag v0.2.0 && git push --tags` 触发 CI，Release 有 4 个平台二进制
- [ ] 下载二进制到干净机器（有 Docker），运行后能提交任务
