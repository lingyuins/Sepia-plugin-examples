# Sepia 示例插件

[English](README.en.md) | 简体中文

一个易读的参考插件，演示 Sepia WASM 插件运行时**每一个 hook 与宿主能力**。它同时是
`backend/tests/plugins.rs` 所用 `echo.wasm` 测试夹具的来源，也可以作为你自己插件的起点。

## 默认安全

安装并启用本插件**不会**改变流量。每个会改写请求的 hook 都由一个需主动开启的配置开关控制；
配置为空时，所有 hook 都原样返回输入。打开 **插件 → Sepia Examples → 配置**，设置某个开关
即可启用对应行为：

| 配置键 | 类型 | 作用 |
| --- | --- | --- |
| `model` | string | 在路由之前改写入站 model |
| `system` | string | 在路由之前覆盖系统提示词 |
| `max_tokens` | integer | 在路由之前限制 `max_tokens`（0 = 保持原值） |
| `exclude` | array | 从路由中排除的供应商 id，例如 `[2, 5]` |
| `reverse_candidates` | boolean | 反转候选顺序（strategy hook 演示） |
| `add_header` | boolean | 向上游请求添加 `x-sepia-plugin: example` 头 |
| `response_marker` | string | 在每个响应 choice 的 content 末尾追加标记 |

`relay.completed`（kv 计数器）以及插件型供应商 hook（`provider.models` / `provider.fetch`）
属于观测/合成类能力，始终运行。

## 构建与打包

```sh
cargo build --target wasm32-unknown-unknown --release
node package.mjs        # -> sepia-examples.zip
```

从插件页（管理员）安装 `sepia-examples.zip`，启用它，然后打开其沙箱化 UI 试用 RPC 桥。

## 沙箱化 UI

`ui/index.html` 运行在 `sandbox="allow-scripts"` 的 iframe 中，受严格 CSP 约束，自身没有
网络访问能力。每个按钮都通过 `window.sepia.call(method, params)` 调用 guest，演示一种宿主能力：

- **count** → `kv`（读取完成计数器）
- **store-secret** → `secret`（加密一个值，再读回）
- **fetch** → `http`（访问 `manifest.http_origins` 中的源）
- **echo** → `log` + 回显（写入一行宿主日志，返回请求体）

## 目录结构

- `src/lib.rs` —— guest 的 `dispatch`，每个 hook 一个带注释的函数。
- `manifest.json` —— 事件、权限、`config_schema`、UI 入口。
- `ui/index.html` —— 沙箱化 RPC 演示。
- `package.mjs` —— 极简 stored-zip 打包脚本（无依赖）。

SDK 位于 Sepia 主仓库的 `backend/plugin-sdk`（no_std，wasm32-unknown-unknown）。本仓库把它
作为 git 依赖引用并钉住提交 `e912ca66`，因此**独立克隆即可构建**，无需同级检出 Sepia。

如果你在 Sepia 检出内开发（`plugin-examples/` 与 `backend/plugin-sdk` 并列），可放一个本地
`.cargo/config.toml` 让构建改用本地 SDK（离线、免拉取；该文件已被 `.gitignore` 忽略，不随仓库分发）：

```toml
paths = ["../backend/plugin-sdk"]
```

这里用 `paths` 而不是 `[patch]`：它只在本机生效，且不会把已提交 `Cargo.lock` 中的 git 源改成本地路径。
升级 SDK 时，把 `Cargo.toml` 里的 `rev` 换成新的 Sepia 提交即可。