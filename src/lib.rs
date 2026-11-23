//! Reference Sepia WASM plugin — a readable tour of every hook and host
//! capability, with **safe-by-default** behavior.
//!
//! Build (from this folder):
//!   cargo build --target wasm32-unknown-unknown --release
//!   node package.mjs        # -> sepia-examples.zip, install via Plugins page
//!
//! Safety: installing and enabling this plugin does NOT change traffic. Each
//! rewriting hook is gated behind an opt-in config flag (see manifest.json
//! `config_schema`); with an empty config every hook returns the input
//! unchanged. Flip a flag in Plugins → Configure to activate that behavior.
//!
//! Events handled:
//!   relay.request.before_route   rewrite model/system/max_tokens   [config.model / system / max_tokens]
//!   relay.route.filter           exclude provider ids             [config.exclude]
//!   relay.strategy               reverse candidate order          [config.reverse_candidates]
//!   relay.request.before_upstream add x-sepia-plugin header        [config.add_header]
//!   relay.response.before_send   append marker to content         [config.response_marker]
//!   relay.completed              count completions in kv          (always, observation only)
//!   provider.models              synthetic catalog                (plugin-backed providers)
//!   provider.fetch               synthetic chat response          (plugin-backed providers)
//!   http.rpc                     echo / count / secret / http demo (sandboxed UI bridge)
//!   plugin.migrate               kv schema bump on upgrade
//!
//! Host capabilities demonstrated: kv, secret, http, log.

#![no_std]
extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use sepia_plugin_sdk::json::Value;
use sepia_plugin_sdk::{http, kv_get, kv_set, log, secret_get, secret_set};

#[no_mangle]
pub extern "Rust" fn dispatch(event: &str, payload: &Value, config: &Value) -> Value {
    match event {
        "relay.request.before_route" => before_route(payload, config),
        "relay.route.filter" => route_filter(config),
        "relay.strategy" => strategy(payload, config),
        "relay.request.before_upstream" => before_upstream(config),
        "relay.response.before_send" => before_send(payload, config),
        "relay.completed" => completed(payload),
        "provider.models" => provider_models(),
        "provider.fetch" => provider_fetch(payload),
        "http.rpc" => rpc(payload),
        "plugin.migrate" => migrate(payload),
        _ => Value::object(),
    }
}

// ---------------------------------------------------------------------------
// Relay rewrite hooks. Each returns the input unchanged unless its config
// flag is set, so an enabled-but-unconfigured plugin is a transparent no-op.
// ---------------------------------------------------------------------------

/// Optionally rewrite the inbound request before routing. Only the fields the
/// host understands (model/system/max_tokens/…) are touched; everything else
/// is preserved by returning the original request object.
fn before_route(payload: &Value, config: &Value) -> Value {
    let mut request = payload.get("request").cloned().unwrap_or(Value::object());
    if let Some(model) = config.get("model").and_then(Value::as_str) {
        if !model.is_empty() {
            request.insert("model", Value::str(model));
        }
    }
    if let Some(system) = config.get("system").and_then(Value::as_str) {
        if !system.is_empty() {
            request.insert("system", Value::str(system));
        }
    }
    if let Some(max_tokens) = config.get("max_tokens").and_then(Value::as_i64) {
        if max_tokens > 0 {
            request.insert("max_tokens", Value::num(max_tokens));
        }
    }
    let mut out = Value::object();
    out.insert("request", request);
    out
}

/// Exclude configured provider ids from the candidate list. Empty config →
/// no exclusions (the host keeps every candidate).
fn route_filter(config: &Value) -> Value {
    let mut out = Value::object();
    if let Some(exclude) = config.get("exclude").and_then(Value::as_array) {
        if !exclude.is_empty() {
            out.insert("exclude_channel_ids", Value::Arr(exclude.clone()));
        }
    }
    out
}

/// Optionally reverse the candidate order to demonstrate a strategy hook.
/// The host only accepts candidates that still exist, so this is always safe.
fn strategy(payload: &Value, config: &Value) -> Value {
    let mut out = Value::object();
    let reverse = config
        .get("reverse_candidates")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if reverse {
        if let Some(candidates) = payload.get("candidates").and_then(Value::as_array) {
            let mut reordered: Vec<Value> = candidates.clone();
            reordered.reverse();
            out.insert("candidates", Value::Arr(reordered));
        }
    }
    out
}

/// Optionally add a single writable header (`x-*`, content-type or accept).
/// Sensitive headers (authorization, cookie, …) are never writable by plugins.
fn before_upstream(config: &Value) -> Value {
    let mut out = Value::object();
    let add = config
        .get("add_header")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if add {
        let mut header = Value::object();
        header.insert("name", Value::str("x-sepia-plugin"));
        header.insert("value", Value::str("example"));
        out.insert("headers", Value::Arr(alloc::vec![header]));
    }
    out
}

/// Optionally append a marker to each choice's text content. Non-text or
/// missing content is left untouched.
fn before_send(payload: &Value, config: &Value) -> Value {
    let mut response = payload.get("response").cloned().unwrap_or(Value::object());
    let marker = config
        .get("response_marker")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !marker.is_empty() {
        if let Some(Value::Arr(choices)) = response.get("choices").cloned() {
            let mut new_choices = Vec::new();
            for mut choice in choices {
                if let Some(mut message) = choice.get("message").cloned() {
                    if let Some(text) = message.get("content").and_then(Value::as_str) {
                        message.insert("content", Value::str(&format!("{text}{marker}")));
                        choice.insert("message", message);
                    }
                }
                new_choices.push(choice);
            }
            response.insert("choices", Value::Arr(new_choices));
        }
    }
    let mut out = Value::object();
    out.insert("response", response);
    out
}

/// Fire-and-forget observation: count completions in plugin kv. Never affects
/// the request, so it runs regardless of config.
fn completed(payload: &Value) -> Value {
    let count = read_counter();
    kv_set("completions", &(count + 1).to_string());
    let model = payload.get("model").and_then(Value::as_str).unwrap_or("");
    log(&format!("completion #{count} model={model}"));
    Value::object()
}

// ---------------------------------------------------------------------------
// Plugin-backed provider hooks. A provider with plugin_id set delegates model
// discovery and inference here instead of an HTTP upstream.
// ---------------------------------------------------------------------------

/// Return the synthetic model catalog. The host requires a non-empty list.
fn provider_models() -> Value {
    let mut out = Value::object();
    out.insert("models", Value::Arr(alloc::vec![Value::str("plugin-chat")]));
    out
}

/// Produce a raw upstream-protocol chat response. The host decodes it through
/// the normal protocol pipeline, so streaming/failover/observability stay
/// consistent. The body is built with the JSON serializer (never string
/// interpolation) so a crafted model name cannot break or inject JSON.
fn provider_fetch(payload: &Value) -> Value {
    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("plugin-chat");

    let mut message = Value::object();
    message.insert("role", Value::str("assistant"));
    message.insert("content", Value::str("plugin pong"));
    let mut choice = Value::object();
    choice.insert("index", Value::num(0));
    choice.insert("message", message);
    choice.insert("finish_reason", Value::str("stop"));
    let mut usage = Value::object();
    usage.insert("prompt_tokens", Value::num(2));
    usage.insert("completion_tokens", Value::num(2));
    let mut body = Value::object();
    body.insert("id", Value::str("chat_plugin"));
    body.insert("model", Value::str(model));
    body.insert("choices", Value::Arr(alloc::vec![choice]));
    body.insert("usage", usage);

    let mut out = Value::object();
    out.insert("status", Value::num(200));
    let mut headers = Value::object();
    headers.insert("content-type", Value::str("application/json"));
    out.insert("headers", headers);
    let body_text = String::from_utf8(body.to_bytes()).unwrap_or_default();
    out.insert("body", Value::str(&body_text));
    out
}

// ---------------------------------------------------------------------------
// RPC bridge for the sandboxed UI (window.sepia.call). Each path demonstrates
// one host capability. Paths are matched exactly; unknown paths echo the body.
// ---------------------------------------------------------------------------

fn rpc(payload: &Value) -> Value {
    let path = payload.get("path").and_then(Value::as_str).unwrap_or("");
    let body = payload.get("body").cloned().unwrap_or(Value::Null);
    match path {
        // kv: read the completion counter maintained by relay.completed.
        "count" => {
            let mut out = Value::object();
            out.insert("completions", Value::str(&read_counter().to_string()));
            out
        }
        // secret: store an encrypted value, then read it back to prove the
        // round-trip. Secrets are encrypted by the gateway KeyStore.
        "store-secret" => {
            let value = body.get("value").and_then(Value::as_str).unwrap_or("");
            secret_set("demo", value);
            let stored = secret_get("demo")
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .unwrap_or_default();
            let mut out = Value::object();
            out.insert("stored", Value::Bool(true));
            out.insert("readback", Value::str(&stored));
            out
        }
        // http: fetch an origin declared in manifest.http_origins. Without an
        // allow-listed origin this returns a clear error instead of panicking.
        "fetch" => {
            let url = body.get("url").and_then(Value::as_str).unwrap_or("");
            let mut out = Value::object();
            match http(url, "GET", None) {
                Ok(result) => {
                    out.insert("ok", Value::Bool(true));
                    out.insert("result", result);
                }
                Err(message) => {
                    out.insert("ok", Value::Bool(false));
                    out.insert("error", Value::str(&message));
                }
            }
            out
        }
        // log + echo: write a host log line and return the request body.
        _ => {
            log(&format!("rpc {path}"));
            let mut out = Value::object();
            out.insert("echo", body);
            out.insert("path", Value::str(path));
            out
        }
    }
}

/// Upgrade hook: bump the kv schema marker. `from_version` is provided by the
/// host so a plugin can migrate stored state across versions.
fn migrate(payload: &Value) -> Value {
    let from = payload
        .get("from_version")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    kv_set("schema", "2");
    log(&format!("migrated from {from}"));
    Value::object()
}

fn read_counter() -> i64 {
    kv_get("completions")
        .and_then(|bytes| core::str::from_utf8(&bytes).ok().and_then(|s| s.parse::<i64>().ok()))
        .unwrap_or(0)
}