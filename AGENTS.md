# AGENTS.md — CCodeBoX 开发规范

## 构建与验证

```bash
# 后端
cd backend && cargo check && cargo clippy  # 0 warning
cargo fmt                                   # 格式化
cargo test                                  # 跑测试

# 前端
cd frontend && npm run build               # 编译检查
```

每次改完都跑上面的命令，不要等最后。

## 代码规范

### Rust
- 错误处理：`anyhow::Result`（应用层）、`thiserror`（库层）
- 异步用 tokio，不用 block_on
- commit 格式：`type(scope): description`（feat/fix/refactor/docs/chore）
- 不留 TODO 注释、不留注释掉的代码块

### TypeScript
- 严格模式（`strict: true`）
- 组件用函数式 + hooks
- API 调用统一走 `lib/api.ts`

## TDD 流程
1. 先写测试（按功能模块粒度），跑 `cargo test` 确认编译通过但断言失败
2. 实现代码，`cargo test` 确认绿灯
3. 需要时重构，保持绿灯

## 隐式契约防御

**必读**：[ImplicitContractDefense.md](./ImplicitContractDefense.md)

所有跨边界交互收敛到隔离仓。每次改完跑 `scripts/check_contracts.sh`。

## 实施约束

1. 不做向后兼容——全新项目，没有旧代码
2. 不加多余抽象——先跑通再优化，不提前写 trait/interface
3. 删掉不用的代码——不留死代码
4. 一次只做一件事——按 Exec Plan 步骤顺序执行

## 设计文档

任务执行前先读相关设计文档：
- `docs/phase1-exec-plan.md` — Phase 1 原始设计
- `docs/phase1-supplement.md` — Phase 1 修正（**冲突时以此为准**）
