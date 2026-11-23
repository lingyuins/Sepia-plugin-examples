# Sepia Examples Plugin

A readable reference plugin that exercises **every hook and host capability** of
the Sepia WASM plugin runtime. It is the source of the `echo.wasm` fixture used
by `backend/tests/plugins.rs`, and a starting point for your own plugins.

## Safe by default

Installing and enabling this plugin does **not** change traffic. Each rewriting
hook is gated behind an opt-in config flag; with an empty config every hook
returns its input unchanged. Open **Plugins → Sepia Examples → Configure** and
set a flag to activate that behavior:

| Config key | Type | Effect |
| --- | --- | --- |
| `model` | string | Rewrite the inbound model before routing |
| `system` | string | Override the system prompt before routing |
| `max_tokens` | integer | Cap `max_tokens` before routing (0 = keep) |
| `exclude` | array | Provider ids to exclude from routing, e.g. `[2, 5]` |
| `reverse_candidates` | boolean | Reverse candidate order (strategy hook demo) |
| `add_header` | boolean | Add `x-sepia-plugin: example` to upstream requests |
| `response_marker` | string | Append a marker to each response choice's content |

`relay.completed` (kv counter) and the plugin-backed provider hooks
(`provider.models` / `provider.fetch`) are observation/synthetic and always run.

## Build & package

```sh
cargo build --target wasm32-unknown-unknown --release
node package.mjs        # -> sepia-examples.zip
```

Install `sepia-examples.zip` from the Plugins page (admin), enable it, then open
its sandboxed UI to try the RPC bridge.

## Sandboxed UI

`ui/index.html` runs in a `sandbox="allow-scripts"` iframe with a strict CSP and
no network access of its own. Every button calls the guest through
`window.sepia.call(method, params)`, demonstrating one host capability:

- **count** → `kv` (reads the completion counter)
- **store-secret** → `secret` (encrypts a value, reads it back)
- **fetch** → `http` (fetches an origin in `manifest.http_origins`)
- **echo** → `log` + echo (writes a host log line, returns the body)

## Layout

- `src/lib.rs` — guest `dispatch`, one commented function per hook.
- `manifest.json` — events, permissions, `config_schema`, UI entry.
- `ui/index.html` — sandboxed RPC demo.
- `package.mjs` — minimal stored-zip packager (no dependencies).

The SDK lives in `../backend/plugin-sdk` (no_std, wasm32-unknown-unknown). This
crate is standalone and can be split into its own repository; only the SDK path
dependency needs to be repointed.