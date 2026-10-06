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
their reachable owned nested types. Unannotated `async-openai` types are
explicit import slots, not fabricated object schemas. This feature does not
require schema derives in `async-openai`, enable an HTTP client, or change
Serde behavior.
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

Owned components use a `dynamo_protocols` namespace. Import slots use an
`async_openai` namespace and carry `x-dynamo-schema-import` with the crate and
type name. Consumers must resolve these slots from a pinned OpenAI specification,
apply reviewed spec-to-code corrections, and verify referenced definitions before
claiming complete coverage. An unresolved marker is a coverage gap, not a schema
that accepts arbitrary inputs. Raw export tests check owned shapes only;
composition fidelity requires additional tests in the consumer.
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
