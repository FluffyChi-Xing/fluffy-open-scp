<!-- PR 标题请遵循 Conventional Commits：<type>(<scope>): <描述> -->
<!-- PR title must follow Conventional Commits: <type>(<scope>): <description> -->

## 变更内容 / What

<!-- 一段话说明这个 PR 做了什么 / One paragraph describing what this PR does -->

## 动机与关联 / Motivation & Links

<!-- 关联 Issue：Fixes #123；逆向结论类变更请注明证据出处（docs/roadmap/migration.md 章节或探针输出） -->
<!-- Link related issues; for reverse-engineering changes, cite the evidence (migration.md section or probe output) -->

## 实现要点 / Implementation Notes

<!-- 关键设计取舍、影响面 / Key trade-offs and blast radius -->

## 自测证据 / How Tested

<!-- 勾选你实际跑过且通过的项 / Check only what you actually ran and passed -->

- [ ] `npm run check`（vue-tsc 类型检查）
- [ ] `npm test`（vitest）
- [ ] `cargo test -p sc-properties --lib`
- [ ] `cargo check`（src-tauri）

结果摘要 / Summary of results:

## 贡献者自查 / Contributor Checklist

- [ ] 提交遵循 [Conventional Commits](https://www.conventionalcommits.org/)，单个提交逻辑独立
- [ ] 不包含游戏原版资源 / `tmp/` 产物 / 存档数据 / `.context/` / `__pycache__`
- [ ] 探针输出只写 `tmp/`；逆向结论已落档 `docs/`（如涉及）
- [ ] UI 文案已同时补充 zh-CN 与 en-US（`src/locales/index.ts`，如涉及）
- [ ] 已基于最新 `master`（不基于其他功能分支）

## 审阅提示 / Reviewer Notes

<!-- 需要重点审阅的文件或模块 / Files or modules that deserve extra attention -->

---

> 提交即表示你同意以 [MIT License](./LICENSE) 许可你的贡献，并遵守[行为准则](./CODE_OF_CONDUCT.md)。
> By submitting you agree to license your work under the [MIT License](./LICENSE) and to abide by the [Code of Conduct](./CODE_OF_CONDUCT.md).
