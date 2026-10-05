// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#![cfg(feature = "protocol-schema")]

use dynamo_protocols::types::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use utoipa::{OpenApi, ToSchema};

#[derive(OpenApi)]
#[openapi(components(schemas(
    CreateChatCompletionRequest,
    CreateCompletionRequest,
    CreateChatCompletionResponse,
    CreateChatCompletionStreamResponse,
    CreateCompletionResponse,
    async_openai::types::chat::CreateChatCompletionRequest,
    async_openai::types::chat::CreateChatCompletionResponse
)))]
struct Contract;

fn document() -> Value {
    serde_json::to_value(Contract::openapi()).unwrap()
}

fn validator<T: ToSchema>() -> jsonschema::Validator {
    let mut doc = document();
    doc["$ref"] = json!(format!("#/components/schemas/{}", T::name()));
    jsonschema::validator_for(&doc).unwrap()
}

fn canonical<T: ToSchema + DeserializeOwned + Serialize>(input: Value) {
    let parsed: T = serde_json::from_value(input).unwrap();
    let output = serde_json::to_value(parsed).unwrap();
    let schema = validator::<T>();
    let errors: Vec<_> = schema.iter_errors(&output).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{output}: {errors:?}");
}

#[test]
fn request_roots_are_distinct_from_upstream_and_have_real_fields() {
    let doc = document();
    let schemas = &doc["components"]["schemas"];
    assert_ne!(
        CreateChatCompletionRequest::name(),
        async_openai::types::chat::CreateChatCompletionRequest::name()
    );
    let own = &schemas[CreateChatCompletionRequest::name().as_ref()];
    assert!(own["properties"].get("messages").is_some());
    assert!(own["properties"].get("mm_processor_kwargs").is_some());
    assert_eq!(own["required"], json!(["messages", "model"]));
    let completion = &schemas[CreateCompletionRequest::name().as_ref()];
    assert!(completion["properties"].get("prompt_embeds").is_some());
    assert!(completion["properties"].get("prompt").is_some());
}

#[test]
fn component_graph_resolves_every_reference() {
    fn walk(value: &Value, root: &Value) {
        match value {
            Value::Object(fields) => {
                if let Some(reference) = fields.get("$ref").and_then(Value::as_str) {
                    assert!(reference.starts_with("#/components/schemas/"));
                    assert!(root.pointer(&reference[1..]).is_some(), "{reference}");
                }
                for child in fields.values() {
                    walk(child, root);
                }
            }
            Value::Array(values) => {
                for child in values {
                    walk(child, root);
                }
            }
            _ => {}
        }
    }
    let doc = document();
    walk(&doc, &doc);
    for name in doc["components"]["schemas"].as_object().unwrap().keys() {
        assert!(
            name.starts_with("dynamo_protocols.") || name.starts_with("async_openai."),
            "{name}"
        );
    }
}

#[test]
fn named_and_string_tool_choices_have_no_variant_wrapper() {
    let schema = validator::<ChatCompletionToolChoiceOption>();
    for value in [
        json!("none"),
        json!("auto"),
        json!("required"),
        json!({"type":"function","function":{"name":"lookup"}}),
    ] {
        assert!(schema.is_valid(&value), "{value}");
        canonical::<ChatCompletionToolChoiceOption>(value);
    }
    assert!(!schema.is_valid(&json!({"Named":{"type":"function","function":{"name":"lookup"}}})));
    assert!(!schema.is_valid(&json!("invalid")));
}

#[test]
fn custom_deserializers_produce_schema_valid_canonical_messages() {
    // These accepted input forms normalize before serialization. The schema
    // describes the canonical shape, not every custom input/alias branch.
    canonical::<CreateChatCompletionRequest>(json!({
        "model":"test",
        "messages":[
            {"role":"system","tools":[{"name":"lookup"}]},
            {"role":"assistant","reasoning":"thinking","content":null,
             "tool_calls":[{"id":"call1","function":{"name":"lookup","arguments":{"q":"hi"}}}]}
        ],
        "stream_options":{"include_usage":null,"continuous_usage_stats":null}
    }));
}

#[test]
fn multimodal_request_and_completion_preserve_native_shapes() {
    canonical::<CreateChatCompletionRequest>(json!({
        "model":"test","messages":[{"role":"user","content":[
            {"type":"text","text":"describe"},
            {"type":"image_url","image_url":{"url":"https://example.com/image.png"}}
        ]}],"mm_processor_kwargs":{"size":256},"reasoning_effort":"high"
    }));
    canonical::<CreateCompletionRequest>(json!({
        "model":"test","prompt":[],"echo":true,"stream_options":{"include_usage":true}
    }));
    let schema = validator::<CreateCompletionRequest>();
    for echo in [json!(1), json!("true")] {
        let request = json!({"model":"test","prompt":"hi","echo":echo});
        assert!(!schema.is_valid(&request));
        assert!(serde_json::from_value::<CreateCompletionRequest>(request).is_err());
    }
}

#[test]
fn unary_and_stream_responses_retain_reasoning_and_usage() {
    canonical::<CreateChatCompletionResponse>(json!({
        "id":"chat-1","object":"chat.completion","created":1,"model":"test",
        "choices":[{"index":0,"message":{"role":"assistant","content":"hi",
                    "reasoning_content":"thought"},"finish_reason":"stop","logprobs":null}]
    }));
    canonical::<CreateChatCompletionStreamResponse>(json!({
        "id":"chat-1","object":"chat.completion.chunk","created":1,"model":"test",
        "choices":[{"index":0,"delta":{"content":"hi","reasoning_content":"thought"},
                    "finish_reason":null,"logprobs":null}]
    }));
}
