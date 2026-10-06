// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Explicit import slots for unannotated async-openai types.
//!
//! These are schema-only stand-ins, never runtime protocol types. Consumers
//! must resolve every `x-dynamo-schema-import` using a version-aligned OpenAI
//! specification before claiming complete schema coverage. The raw export is
//! deliberately marked rather than claiming that an arbitrary object is valid.

macro_rules! imports {
    ($($name:ident),* $(,)?) => {$(
        pub struct $name;

        impl utoipa::PartialSchema for $name {
            fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
                use utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};
                ObjectBuilder::new()
                    // These imported Rust types are non-Option. Excluding null
                    // prevents overlap with utoipa's Option<T> oneOf null arm.
                    // The marker remains incomplete until composition.
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
