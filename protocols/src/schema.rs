// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Schema-only request adapters and placeholders for upstream types.
//!
//! Request adapters describe accepted function arguments and reasoning aliases
//! without widening shared response schemas or changing runtime Serde behavior.
//! The assistant request adapter owns its field schemas because its alias rule
//! applies to the whole object; keep it aligned when that runtime type changes.
//!
//! These placeholders let Dynamo-owned types derive `ToSchema` without
//! requiring an upstream schema feature. Field-level `schema(value_type = ...)`
//! annotations select them for schema generation only; runtime field types and
//! Serde behavior remain unchanged.
//!
//! Each placeholder preserves the upstream type's identity through an
//! `x-dynamo-schema-import` marker, but does not describe its detailed shape.
//! Consumers must resolve these markers against version-aligned upstream
//! schemas before claiming complete coverage. This module does not perform
//! that resolution or merge specifications.
//!
//! Once native upstream schema support is integrated, the corresponding
//! placeholders and field overrides can be removed.

use crate::types::{ChatCompletionRequestAssistantMessage, ReasoningContent};
use utoipa::openapi::schema::{AllOfBuilder, AnyOfBuilder, ObjectBuilder, Type};
use utoipa::openapi::{RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};

fn input_arguments() -> RefOr<Schema> {
    AnyOfBuilder::new()
        .item(String::schema())
        .item(ObjectBuilder::new().schema_type(Type::Object))
        .into()
}

#[derive(ToSchema)]
#[schema(as = dynamo_protocols::chat::RequestFunctionCall)]
struct RequestFunctionCall {
    name: String,
    #[schema(schema_with = input_arguments)]
    arguments: String,
}

#[derive(ToSchema)]
#[schema(as = dynamo_protocols::chat::RequestToolCall)]
struct RequestToolCall {
    id: String,
    // Serde supplies the function tag when it is absent on input.
    #[schema(required = false)]
    r#type: FunctionType,
    function: RequestFunctionCall,
}

#[derive(ToSchema)]
#[schema(as = dynamo_protocols::chat::ChatCompletionRequestAssistantMessage)]
struct AssistantRequestFields {
    content: Option<ChatCompletionRequestAssistantMessageContent>,
    reasoning_content: Option<ReasoningContent>,
    refusal: Option<String>,
    name: Option<String>,
    audio: Option<ChatCompletionRequestAssistantMessageAudio>,
    tool_calls: Option<Vec<RequestToolCall>>,
    #[deprecated]
    function_call: Option<RequestFunctionCall>,
    partial: Option<bool>,
}

impl PartialSchema for ChatCompletionRequestAssistantMessage {
    fn schema() -> RefOr<Schema> {
        let RefOr::T(Schema::Object(mut fields)) = AssistantRequestFields::schema() else {
            unreachable!("assistant field schema must be an object");
        };
        let reasoning = fields.properties["reasoning_content"].clone();
        fields.properties.insert("reasoning".into(), reasoning);

        // Utoipa 5.5 has no `not` or boolean-schema representation. No value
        // can be both string and null, so this is a standard always-false
        // property schema. Requiring at least one spelling to be absent is
        // equivalent to not: {required: [reasoning, reasoning_content]}.
        // Unlike injecting `not` via Extensions, it survives typed reloads.
        let forbidden: RefOr<Schema> = AllOfBuilder::new()
            .item(ObjectBuilder::new().schema_type(Type::String))
            .item(ObjectBuilder::new().schema_type(Type::Null))
            .into();
        AllOfBuilder::new()
            .item(fields)
            .item(
                AnyOfBuilder::new()
                    .item(ObjectBuilder::new().property("reasoning", forbidden.clone()))
                    .item(ObjectBuilder::new().property("reasoning_content", forbidden)),
            )
            .into()
    }
}

impl ToSchema for ChatCompletionRequestAssistantMessage {
    fn name() -> std::borrow::Cow<'static, str> {
        AssistantRequestFields::name()
    }

    fn schemas(schemas: &mut Vec<(String, RefOr<Schema>)>) {
        AssistantRequestFields::schemas(schemas);
    }
}

macro_rules! imports {
    ($($name:ident),* $(,)?) => {$(
        pub struct $name;

        impl utoipa::PartialSchema for $name {
            fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
                use utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};
                ObjectBuilder::new()
                    // These imported Rust types are non-Option. Excluding null
                    // prevents overlap with utoipa's Option<T> oneOf null arm.
                    .schema_type(SchemaType::Array(vec![
                        Type::Object, Type::Array, Type::String,
                        Type::Number, Type::Boolean,
                    ]))
                    .extensions(Some([(
                        "x-dynamo-schema-import",
                        serde_json::json!({"crate": "async-openai", "type": stringify!($name)}),
                    )].into_iter().collect()))
                    .into()
            }
        }

        impl utoipa::ToSchema for $name {
            fn name() -> std::borrow::Cow<'static, str> {
                concat!("async_openai.", stringify!($name)).into()
            }
        }
    )*};
}

imports!(
    FunctionType,
    ImageDetail,
    TopLogprobs,
    FunctionObject,
    ChatCompletionRequestMessageContentPartText,
    ChatCompletionRequestMessageContentPartAudio,
    ChatCompletionRequestSystemMessageContent,
    ChatCompletionRequestAssistantMessageContent,
    ChatCompletionRequestAssistantMessageAudio,
    ChatCompletionRequestDeveloperMessage,
    ChatCompletionRequestFunctionMessage,
    Role,
    ChatCompletionResponseMessageAudio,
    ResponseModalities,
    PredictionContent,
    ChatCompletionAudio,
    ResponseFormat,
    ServiceTier,
    ChatCompletionFunctionCall,
    ChatCompletionFunctions,
    WebSearchOptions,
    FinishReason,
    CompletionUsage,
    Prompt,
);
