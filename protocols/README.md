# dynamo-protocols

Request/response types for OpenAI- and Anthropic-compatible inference servers. Built on [`async-openai`](https://crates.io/crates/async-openai) v0.34, with selective overrides where inference engines need behaviors upstream doesn't support.

## What's included

- **Chat Completions** — multimodal content, reasoning content (DeepSeek-R1 / QwQ), continuous usage stats, tool-calling.
- **Batch API** — re-exported from upstream.
- **Files API** — re-exported from upstream.
- **Responses API** — input chain owned (relaxed for Codex / Agents SDK); output chain re-exported from upstream.
- **Completions** — re-exported.
- **Anthropic Messages** — fully owned (no upstream equivalent).
- **Embeddings**, **Images** — re-exported.

## Optional protocol schemas

The opt-in `protocol-schema` feature supplies utoipa 5 `ToSchema`
implementations for chat/completion requests, responses, stream chunks, and
their reachable nested types. It forwards the schema feature to
`async-openai`; it does not enable an HTTP client or change Serde behavior.
Other endpoint families are outside this initial schema surface.

```rust,ignore
#[derive(utoipa::OpenApi)]
#[openapi(components(schemas(
    dynamo_protocols::types::CreateChatCompletionRequest,
    dynamo_protocols::types::CreateCompletionRequest,
    dynamo_protocols::types::CreateChatCompletionResponse,
    dynamo_protocols::types::CreateChatCompletionStreamResponse
)))]
struct Contract;
```

Owned components use a `dynamo_protocols` namespace, so upstream
`async_openai` types can coexist in the same document without name collisions.
The schemas describe canonical typed/serialized forms, **not a complete
input-validation contract**. Custom deserializers additionally accept input
aliases (`reasoning`), object-valued function arguments, media shorthand,
tools-only system messages, and null stream-option booleans. Some input
combinations are rejected by role-specific validation. These behaviors still
need explicit coverage when using the schemas for compatibility assessment;
a schema comparison alone must not label them equivalent.

Run `cargo test -p dynamo-protocols --features protocol-schema` to exercise
schema generation and existing protocol tests.

## Locally-defined extensions

A few fields extend the upstream `async-openai` schema:

- `reasoning_content` on assistant messages
- `mm_processor_kwargs` on chat-completion requests (vLLM multimodal)
- `continuous_usage_stats` on chat stream options
- `FunctionCall.arguments` accepts both string and object forms

## Request compatibility and serving policy

`CreateResponse` accepts `"text":{"verbosity":"low"}` without a `format`.
The upstream `ResponseTextParam` defaults the format to `text`. An explicit
`text`, `json_object`, or `json_schema` format is preserved independently of
verbosity. A serving adapter that cannot honor verbosity should ignore that
hint while retaining the output-format constraint. Map supported fields to
the engine request instead of forwarding the Responses `text` object.

`CreateChatCompletionRequest` and `CreateCompletionRequest` accept and preserve
`stream_options` when `stream` is absent, null, false, or true. These types do
not enforce cross-field streaming policy. A serving adapter that chooses
permissive compatibility should clear `stream_options` when
`request.stream != Some(true)` before backend validation or forwarding. Keep
the options when streaming, and keep normal non-streaming response usage.
This normalization belongs in the serving adapter because protocol types
also serve callers that need to preserve the original request.

Run the wire-level regressions for
[#299](https://github.com/ai-dynamo/frontend-crates/issues/299) with
`cargo test -p dynamo-protocols --test request_compatibility`.
