# 编码规范

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
