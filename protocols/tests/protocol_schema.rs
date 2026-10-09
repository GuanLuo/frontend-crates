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
    CreateChatCompletionStreamResponse
)))]
struct Contract;

fn document() -> Value {
    serde_json::to_value(Contract::openapi()).unwrap()
}

fn validator<T: ToSchema>() -> jsonschema::Validator {
    let mut dependencies = Vec::new();
    T::schemas(&mut dependencies);
    let components = utoipa::openapi::schema::ComponentsBuilder::new()
        .schemas_from_iter(dependencies)
        .schema_from::<T>()
        .build();
    let doc = json!({
        "$schema":"https://json-schema.org/draft/2020-12/schema",
        "components":components,
        "$ref":format!("#/components/schemas/{}", T::name()),
    });
    jsonschema::validator_for(&doc).unwrap()
}

// Input fidelity compares the original JSON, before Serde normalizes it.
fn input_fidelity<T: ToSchema + DeserializeOwned>(input: Value, valid: bool) {
    assert_eq!(validator::<T>().is_valid(&input), valid, "schema: {input}");
    assert_eq!(
        serde_json::from_value::<T>(input.clone()).is_ok(),
        valid,
        "Serde: {input}"
    );
}

// Output fidelity checks the serialized result, not acceptance of the input.
fn canonical<T: ToSchema + DeserializeOwned + Serialize>(input: Value) {
    let parsed: T = serde_json::from_value(input).unwrap();
    let output = serde_json::to_value(parsed).unwrap();
    let schema = validator::<T>();
    let errors: Vec<_> = schema.iter_errors(&output).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{output}: {errors:?}");
}

#[test]
fn fidelity_helpers_collect_arbitrary_type_dependencies() {
    #[derive(serde::Deserialize, Serialize, ToSchema)]
    struct Nested {
        value: String,
    }
    #[derive(serde::Deserialize, Serialize, ToSchema)]
    struct Root {
        nested: Nested,
    }
    let input = json!({"nested":{"value":"ok"}});
    input_fidelity::<Root>(input.clone(), true);
    canonical::<Root>(input);
    input_fidelity::<Root>(json!({"nested":{"value":1}}), false);
}

#[test]
fn request_roots_are_distinct_from_upstream_and_have_real_fields() {
    let doc = document();
    let schemas = &doc["components"]["schemas"];
    assert_eq!(
        CreateChatCompletionRequest::name(),
        "dynamo_protocols.chat.CreateChatCompletionRequest"
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
fn unannotated_dependency_types_are_explicit_import_slots() {
    let doc = document();
    let schemas = doc["components"]["schemas"].as_object().unwrap();
    for (name, schema) in schemas {
        if let Some(short) = name.strip_prefix("async_openai.") {
            assert_eq!(schema["x-dynamo-schema-import"]["crate"], "async-openai");
            assert_eq!(schema["x-dynamo-schema-import"]["type"], short);
            let kinds = schema["type"].as_array().unwrap();
            assert!(
                !kinds.contains(&json!("null")),
                "Option<T> owns nullability"
            );
            assert!(
                kinds.len() > 1,
                "must not fabricate an object-only contract"
            );
        }
    }
    assert!(schemas.contains_key("async_openai.Prompt"));
    assert!(schemas.contains_key("async_openai.FunctionObject"));
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

#[test]
fn stop_schema_accepts_empty_arrays_and_preserves_variant_shapes() {
    let chat_schema = validator::<CreateChatCompletionRequest>();
    let completion_schema = validator::<CreateCompletionRequest>();
    for stop in [json!([]), json!("end"), json!(["end"]), json!([576])] {
        let chat = json!({
            "model":"test", "messages":[{"role":"user","content":"hi"}], "stop":stop
        });
        let completion = json!({"model":"test","prompt":"hi","stop":stop});
        assert!(chat_schema.is_valid(&chat), "{chat}");
        assert!(completion_schema.is_valid(&completion), "{completion}");
        canonical::<CreateChatCompletionRequest>(chat);
        canonical::<CreateCompletionRequest>(completion);
    }
    for stop in [json!(["end", 576]), json!([-1]), json!(true)] {
        let chat = json!({
            "model":"test", "messages":[{"role":"user","content":"hi"}], "stop":stop
        });
        let completion = json!({"model":"test","prompt":"hi","stop":stop});
        assert!(!chat_schema.is_valid(&chat), "{chat}");
        assert!(!completion_schema.is_valid(&completion), "{completion}");
        assert!(serde_json::from_value::<CreateChatCompletionRequest>(chat).is_err());
        assert!(serde_json::from_value::<CreateCompletionRequest>(completion).is_err());
    }
}

fn required_nullable_output<T: ToSchema + DeserializeOwned + Serialize>(
    input: Value,
    required_fields: &[(&str, &str)],
    omitted_fields: &[&str],
) {
    let parsed: T = serde_json::from_value(input).unwrap();
    let output = serde_json::to_value(parsed).unwrap();
    let schema = validator::<T>();
    assert!(schema.is_valid(&output), "{output}");
    for &(parent, field) in required_fields {
        assert_eq!(
            output.pointer(parent).unwrap().get(field),
            Some(&Value::Null)
        );
        let mut missing = output.clone();
        missing
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            !schema.is_valid(&missing),
            "missing {parent}/{field}: {missing}"
        );
        // Deserialization remains permissive; this schema describes output.
        assert!(serde_json::from_value::<T>(missing).is_ok());
    }
    for pointer in omitted_fields {
        assert!(output.pointer(pointer).is_none(), "{pointer}: {output}");
    }
}

#[test]
fn unary_response_requires_always_serialized_nullable_fields() {
    required_nullable_output::<CreateChatCompletionResponse>(
        json!({
            "id":"chat-1","object":"chat.completion","created":1,"model":"test",
            "choices":[{"index":0,"message":{"role":"assistant"}}]
        }),
        &[
            ("/choices/0/message", "content"),
            ("/choices/0/message", "refusal"),
            ("/choices/0", "finish_reason"),
            ("/choices/0", "logprobs"),
        ],
        &[
            "/choices/0/message/tool_calls",
            "/choices/0/message/reasoning_content",
            "/usage",
        ],
    );
}

#[test]
fn stream_response_requires_always_serialized_nullable_fields() {
    required_nullable_output::<CreateChatCompletionStreamResponse>(
        json!({
            "id":"chat-1","object":"chat.completion.chunk","created":1,"model":"test",
            "choices":[{"index":0,"delta":{}}]
        }),
        &[("/choices/0", "finish_reason"), ("/choices/0", "logprobs")],
        &[
            "/choices/0/delta/content",
            "/choices/0/delta/refusal",
            "/usage",
        ],
    );
}

#[test]
fn logprobs_require_nullable_content_and_refusal() {
    required_nullable_output::<ChatChoiceLogprobs>(
        json!({}),
        &[("", "content"), ("", "refusal")],
        &[],
    );
    canonical::<ChatChoiceLogprobs>(json!({"content":[],"refusal":[]}));
}

#[test]
fn token_logprobs_require_nullable_bytes_but_not_token_id() {
    required_nullable_output::<ChatCompletionTokenLogprob>(
        json!({"token":"a","logprob":-0.5,"top_logprobs":[]}),
        &[("", "bytes")],
        &["/token_id"],
    );
    for bytes in [json!(null), json!([]), json!([97])] {
        let token = json!({"token":"a","logprob":-0.5,"bytes":bytes,"top_logprobs":[]});
        canonical::<ChatCompletionTokenLogprob>(token.clone());
        let logprobs = json!({"content":[token.clone()],"refusal":[token]});
        canonical::<CreateChatCompletionResponse>(json!({
            "id":"chat-1","object":"chat.completion","created":1,"model":"test",
            "choices":[{"index":0,"message":{"role":"assistant"},"logprobs":logprobs}]
        }));
        canonical::<CreateChatCompletionStreamResponse>(json!({
            "id":"chat-1","object":"chat.completion.chunk","created":1,"model":"test",
            "choices":[{"index":0,"delta":{},"logprobs":logprobs}]
        }));
    }
}

fn assistant_request(message: Value) -> Value {
    json!({"model":"test", "messages":[message]})
}

#[test]
fn request_arguments_accept_strings_or_objects_on_both_paths() {
    for (arguments, valid) in [
        (json!("{\"q\":1}"), true),
        (json!(""), true),
        (json!({}), true),
        (json!({"q":[1,null]}), true),
        (json!([]), false),
        (json!(null), false),
        (json!(1), false),
        (json!(true), false),
    ] {
        for tools in [false, true] {
            let call = json!({"name":"lookup","arguments":arguments});
            let mut message = json!({"role":"assistant"});
            if tools {
                // The type tag has a Serde default and may be omitted on input.
                message["tool_calls"] = json!([{"id":"call1","function":call}]);
            } else {
                message["function_call"] = call;
            }
            let request = assistant_request(message);
            input_fidelity::<CreateChatCompletionRequest>(request.clone(), valid);
            if valid {
                let parsed: CreateChatCompletionRequest = serde_json::from_value(request).unwrap();
                let output = serde_json::to_value(parsed).unwrap();
                let path = if tools {
                    "/messages/0/tool_calls/0/function/arguments"
                } else {
                    "/messages/0/function_call/arguments"
                };
                let expected = arguments
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| arguments.to_string());
                assert_eq!(output.pointer(path), Some(&json!(expected)));
            }
        }
    }
    for call in [json!({"name":"lookup"}), json!({"arguments":{}})] {
        for message in [
            json!({"role":"assistant","function_call":call}),
            json!({"role":"assistant","tool_calls":[{"id":"call1","function":call}]}),
        ] {
            input_fidelity::<CreateChatCompletionRequest>(assistant_request(message), false);
        }
    }
}

#[test]
fn reasoning_alias_has_the_same_input_type_and_canonical_output() {
    for name in ["reasoning", "reasoning_content"] {
        for (value, valid) in [
            (json!(null), true),
            (json!("thought"), true),
            (json!([]), true),
            (json!(["a", "b"]), true),
            (json!(1), false),
            (json!({}), false),
            (json!(false), false),
            (json!(["a", 1]), false),
        ] {
            let message = json!({"role":"assistant", (name):value});
            input_fidelity::<ChatCompletionRequestAssistantMessage>(message.clone(), valid);
            let request = assistant_request(message);
            input_fidelity::<CreateChatCompletionRequest>(request.clone(), valid);
            if valid {
                let parsed: CreateChatCompletionRequest = serde_json::from_value(request).unwrap();
                let output = serde_json::to_value(parsed).unwrap();
                assert!(output["messages"][0].get("reasoning").is_none());
                if value.is_null() {
                    assert!(output["messages"][0].get("reasoning_content").is_none());
                } else {
                    assert_eq!(output["messages"][0]["reasoning_content"], value);
                }
            }
        }
    }
}

#[test]
fn reasoning_aliases_are_mutually_exclusive_even_for_nulls() {
    for (left, right) in [
        (json!("a"), json!("a")),
        (json!("a"), json!("b")),
        (json!(null), json!(null)),
        (json!(null), json!("a")),
        (json!("a"), json!(null)),
    ] {
        let message = json!({"role":"assistant","reasoning":left,"reasoning_content":right});
        input_fidelity::<ChatCompletionRequestAssistantMessage>(message.clone(), false);
        input_fidelity::<CreateChatCompletionRequest>(assistant_request(message), false);
    }
    input_fidelity::<CreateChatCompletionRequest>(
        assistant_request(json!({"role":"assistant","unknown":"still allowed"})),
        true,
    );
}

#[test]
fn response_arguments_remain_string_only() {
    for tools in [false, true] {
        for (arguments, valid) in [(json!("{}"), true), (json!({}), false)] {
            let call = json!({"name":"lookup","arguments":arguments});
            let mut message = json!({"role":"assistant","content":null,"refusal":null});
            if tools {
                message["tool_calls"] = json!([{"id":"call1","type":"function","function":call}]);
            } else {
                message["function_call"] = call;
            }
            let response = json!({"id":"chat-1","object":"chat.completion","created":1,"model":"test",
                "choices":[{"index":0,"message":message,"finish_reason":null,"logprobs":null}]});
            assert_eq!(
                validator::<CreateChatCompletionResponse>().is_valid(&response),
                valid
            );
            canonical::<CreateChatCompletionResponse>(response);
        }
    }
}

#[test]
fn alias_exclusion_survives_typed_openapi_round_trip() {
    let mut doc = document();
    // Isolate the new constraint: unrelated existing components cannot all be
    // deserialized back into Utoipa 5.5's narrower schema representation.
    let constraint = &mut doc["components"]["schemas"]
        [ChatCompletionRequestAssistantMessage::name().as_ref()]["allOf"][1];
    let reloaded: utoipa::openapi::Schema = serde_json::from_value(constraint.clone()).unwrap();
    *constraint = serde_json::to_value(reloaded).unwrap();
    doc["$ref"] = json!(format!(
        "#/components/schemas/{}",
        CreateChatCompletionRequest::name()
    ));
    let schema = jsonschema::validator_for(&doc).unwrap();
    for (message, valid) in [
        (json!({"role":"assistant"}), true),
        (json!({"role":"assistant","reasoning":"a"}), true),
        (json!({"role":"assistant","reasoning_content":"a"}), true),
        (
            json!({"role":"assistant","reasoning":"a","reasoning_content":"a"}),
            false,
        ),
    ] {
        assert_eq!(schema.is_valid(&assistant_request(message)), valid);
    }
}

#[test]
fn assistant_adapter_covers_fields_and_does_not_deprecate_reasoning() {
    let parsed: ChatCompletionRequestAssistantMessage = serde_json::from_value(json!({
        "content":"answer", "reasoning_content":"thought", "refusal":"no", "name":"agent",
        "audio":{"id":"audio1"}, "tool_calls":[{"id":"call1","function":{"name":"f","arguments":"{}"}}],
        "function_call":{"name":"f","arguments":"{}"}, "partial":true,
    })).unwrap();
    let output = serde_json::to_value(parsed).unwrap();
    let doc = document();
    let properties = doc["components"]["schemas"]
        [ChatCompletionRequestAssistantMessage::name().as_ref()]["allOf"][0]["properties"]
        .as_object()
        .unwrap();
    let declared: std::collections::BTreeSet<_> = properties
        .keys()
        .filter(|key| key.as_str() != "reasoning")
        .collect();
    assert_eq!(declared, output.as_object().unwrap().keys().collect());
    assert_eq!(properties["reasoning"], properties["reasoning_content"]);
    assert_ne!(
        properties["reasoning"].get("deprecated"),
        Some(&json!(true))
    );
}

#[test]
fn request_and_response_roots_register_independently() {
    #[derive(OpenApi)]
    #[openapi(components(schemas(CreateChatCompletionRequest)))]
    struct Requests;
    #[derive(OpenApi)]
    #[openapi(components(schemas(
        CreateChatCompletionResponse,
        CreateChatCompletionStreamResponse
    )))]
    struct Responses;

    let request = serde_json::to_value(Requests::openapi()).unwrap();
    let response = serde_json::to_value(Responses::openapi()).unwrap();
    let combined = document();
    for doc in [&request, &response] {
        for (name, schema) in doc["components"]["schemas"].as_object().unwrap() {
            assert_eq!(schema, &combined["components"]["schemas"][name], "{name}");
        }
    }
    assert!(
        request["components"]["schemas"]
            .get("dynamo_protocols.chat.RequestFunctionCall")
            .is_some()
    );
    assert!(
        response["components"]["schemas"]
            .get("dynamo_protocols.chat.RequestFunctionCall")
            .is_none()
    );
    let message =
        &response["components"]["schemas"]["dynamo_protocols.chat.ChatCompletionResponseMessage"];
    assert!(message["properties"].get("reasoning").is_none());
    let mut request = request;
    request["$ref"] = json!(format!(
        "#/components/schemas/{}",
        CreateChatCompletionRequest::name()
    ));
    let schema = jsonschema::validator_for(&request).unwrap();
    assert!(schema.is_valid(&assistant_request(
        json!({"role":"assistant", "reasoning":"a",
        "tool_calls":[{"id":"call1","function":{"name":"f","arguments":{}}}]})
    )));
}
