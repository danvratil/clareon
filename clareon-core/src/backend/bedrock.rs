// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! AWS Bedrock backend implementation

use std::pin::Pin;
use std::sync::LazyLock;

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::error::SdkError;
use aws_sdk_bedrockruntime::types::{
    CachePointBlock, CachePointType, ContentBlock as BedrockContentBlock, ContentBlockDelta,
    ContentBlockStart as BedrockContentBlockStart, ConversationRole, ConverseStreamOutput,
    Message as BedrockMessage, SystemContentBlock, Tool as BedrockTool, ToolConfiguration,
    ToolInputSchema, ToolResultBlock, ToolResultContentBlock, ToolUseBlock,
};
use aws_smithy_types::Document;
use futures::Stream;
use tracing::{debug, info, warn};

use super::traits::{
    ChatRequest, ChatResponse, ContentDelta, LlmBackend, ModelInfo, StopReason, StreamEvent, Usage,
};
use crate::error::BackendError;
use crate::types::{ContentBlock, ConversationId, Message, Role, ToolResultContent};

/// AWS Bedrock backend for Anthropic models
pub struct BedrockBackend {
    client: Client,
    region: String,
    enable_prompt_caching: bool,
}

impl BedrockBackend {
    /// Create a new Bedrock backend using the default credential chain
    pub async fn new(region: impl Into<String>) -> Result<Self, BackendError> {
        Self::new_with_config(region, None, true).await
    }

    /// Create a new Bedrock backend with a specific AWS profile
    pub async fn with_profile(
        region: impl Into<String>,
        profile: impl Into<String>,
    ) -> Result<Self, BackendError> {
        Self::new_with_config(region, Some(profile.into()), true).await
    }

    /// Create a new Bedrock backend with full configuration
    pub async fn new_with_config(
        region: impl Into<String>,
        profile: Option<String>,
        enable_prompt_caching: bool,
    ) -> Result<Self, BackendError> {
        let region = region.into();
        info!(
            "Initializing Bedrock backend in region: {} (caching: {})",
            region, enable_prompt_caching
        );

        let mut config_loader = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_config::Region::new(region.clone()));

        if let Some(prof) = &profile {
            config_loader = config_loader.profile_name(prof);
        }

        let config = config_loader.load().await;
        let client = Client::new(&config);

        Ok(Self {
            client,
            region,
            enable_prompt_caching,
        })
    }

    /// Convert serde_json::Value to AWS Document
    fn json_to_document(value: &serde_json::Value) -> Document {
        match value {
            serde_json::Value::Null => Document::Null,
            serde_json::Value::Bool(b) => Document::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Document::Number(aws_smithy_types::Number::NegInt(i))
                } else if let Some(u) = n.as_u64() {
                    Document::Number(aws_smithy_types::Number::PosInt(u))
                } else if let Some(f) = n.as_f64() {
                    Document::Number(aws_smithy_types::Number::Float(f))
                } else {
                    Document::Null
                }
            }
            serde_json::Value::String(s) => Document::String(s.clone()),
            serde_json::Value::Array(arr) => {
                Document::Array(arr.iter().map(Self::json_to_document).collect())
            }
            serde_json::Value::Object(obj) => Document::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), Self::json_to_document(v)))
                    .collect(),
            ),
        }
    }

    /// Convert AWS Document to serde_json::Value
    fn document_to_json(doc: &Document) -> serde_json::Value {
        match doc {
            Document::Null => serde_json::Value::Null,
            Document::Bool(b) => serde_json::Value::Bool(*b),
            Document::Number(n) => match n {
                aws_smithy_types::Number::PosInt(i) => serde_json::json!(*i),
                aws_smithy_types::Number::NegInt(i) => serde_json::json!(*i),
                aws_smithy_types::Number::Float(f) => serde_json::Number::from_f64(*f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
            },
            Document::String(s) => serde_json::Value::String(s.clone()),
            Document::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(Self::document_to_json).collect())
            }
            Document::Object(obj) => serde_json::Value::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), Self::document_to_json(v)))
                    .collect(),
            ),
        }
    }

    /// Convert our Message type to Bedrock API format
    fn convert_messages(messages: &[Message]) -> Result<Vec<BedrockMessage>, BackendError> {
        messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    Role::User => ConversationRole::User,
                    Role::Assistant => ConversationRole::Assistant,
                };

                let content: Vec<BedrockContentBlock> = msg
                    .content
                    .iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => BedrockContentBlock::Text(text.clone()),
                        ContentBlock::Image { source } => {
                            use aws_sdk_bedrockruntime::types::{
                                ImageBlock, ImageFormat, ImageSource as BedrockImageSource,
                            };

                            match source {
                                crate::types::ImageSource::Base64 { media_type, data } => {
                                    // Decode base64 to bytes
                                    use base64::Engine;
                                    let bytes = base64::engine::general_purpose::STANDARD
                                        .decode(data)
                                        .expect("Invalid base64 data");

                                    // Determine image format from media type
                                    let format = match media_type.as_str() {
                                        "image/jpeg" => ImageFormat::Jpeg,
                                        "image/png" => ImageFormat::Png,
                                        "image/gif" => ImageFormat::Gif,
                                        "image/webp" => ImageFormat::Webp,
                                        _ => {
                                            warn!(
                                                "Unsupported media type {}, defaulting to JPEG",
                                                media_type
                                            );
                                            ImageFormat::Jpeg
                                        }
                                    };

                                    let image_source = BedrockImageSource::Bytes(bytes.into());
                                    let image_block = ImageBlock::builder()
                                        .format(format)
                                        .source(image_source)
                                        .build()
                                        .expect("Failed to build ImageBlock");

                                    BedrockContentBlock::Image(image_block)
                                }
                            }
                        }
                        ContentBlock::ToolUse { id, name, input } => BedrockContentBlock::ToolUse(
                            ToolUseBlock::builder()
                                .tool_use_id(id)
                                .name(name)
                                .input(Self::json_to_document(input))
                                .build()
                                .expect("Failed to build ToolUseBlock"),
                        ),
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => {
                            let result_content: Vec<ToolResultContentBlock> = content
                                .iter()
                                .map(|c| match c {
                                    ToolResultContent::Text { text } => {
                                        ToolResultContentBlock::Text(text.clone())
                                    }
                                })
                                .collect();

                            let mut builder = ToolResultBlock::builder()
                                .tool_use_id(tool_use_id)
                                .set_content(Some(result_content));

                            if is_error.unwrap_or(false) {
                                builder = builder
                                    .status(aws_sdk_bedrockruntime::types::ToolResultStatus::Error);
                            }

                            BedrockContentBlock::ToolResult(
                                builder.build().expect("Failed to build ToolResultBlock"),
                            )
                        }
                    })
                    .collect();

                BedrockMessage::builder()
                    .role(role)
                    .set_content(Some(content))
                    .build()
                    .map_err(|e| BackendError::InvalidResponse(e.to_string()))
            })
            .collect()
    }

    /// Convert Bedrock response content to our ContentBlock type
    fn convert_content(content: &[BedrockContentBlock]) -> Vec<ContentBlock> {
        content
            .iter()
            .filter_map(|block| match block {
                BedrockContentBlock::Text(text) => Some(ContentBlock::Text { text: text.clone() }),
                BedrockContentBlock::ToolUse(tool_use) => Some(ContentBlock::ToolUse {
                    id: tool_use.tool_use_id().to_string(),
                    name: tool_use.name().to_string(),
                    input: Self::document_to_json(tool_use.input()),
                }),
                _ => None, // Ignore other content types for now
            })
            .collect()
    }

    /// Convert our ToolDefinition to Bedrock's ToolConfiguration
    fn convert_tools(tools: &[super::traits::ToolDefinition]) -> Option<ToolConfiguration> {
        if tools.is_empty() {
            return None;
        }

        let bedrock_tools: Vec<BedrockTool> = tools
            .iter()
            .map(|tool| {
                let tool_spec = aws_sdk_bedrockruntime::types::ToolSpecification::builder()
                    .name(&tool.name)
                    .description(&tool.description)
                    .input_schema(ToolInputSchema::Json(Self::json_to_document(
                        &tool.input_schema,
                    )))
                    .build()
                    .expect("Failed to build ToolSpecification");

                BedrockTool::ToolSpec(tool_spec)
            })
            .collect();

        Some(
            ToolConfiguration::builder()
                .set_tools(Some(bedrock_tools))
                .build()
                .expect("Failed to build ToolConfiguration"),
        )
    }

    /// Build system content blocks with optional cache point
    ///
    /// If caching is enabled and the system prompt is non-empty, this returns:
    /// [SystemContentBlock::Text(prompt), SystemContentBlock::CachePoint(default)]
    ///
    /// This tells Bedrock to cache the system prompt for 5 minutes, reducing costs
    /// and latency on subsequent calls within the TTL window.
    fn build_system_blocks(&self, system_prompt: &str) -> Vec<SystemContentBlock> {
        if system_prompt.is_empty() {
            return Vec::new();
        }

        let mut blocks = vec![SystemContentBlock::Text(system_prompt.to_string())];

        if self.enable_prompt_caching {
            // Add cache point after system prompt
            let cache_point = CachePointBlock::builder()
                .r#type(CachePointType::Default)
                .build()
                .expect("Failed to build CachePointBlock");
            blocks.push(SystemContentBlock::CachePoint(cache_point));
        }

        blocks
    }
}

#[async_trait]
impl LlmBackend for BedrockBackend {
    async fn send_message(&self, request: &ChatRequest) -> Result<ChatResponse, BackendError> {
        info!(
            "Sending message to Bedrock, model: {}, region: {}",
            request.model, self.region
        );
        debug!("Message count: {}", request.messages.len());
        debug!("Max tokens: {}", request.max_tokens);
        debug!("Tools count: {}", request.tools.len());
        if let Some(sys) = &request.system_prompt {
            debug!("System prompt length: {} chars", sys.len());
        }
        for (i, msg) in request.messages.iter().enumerate() {
            debug!(
                "Message {}: role={:?}, content_blocks={}",
                i,
                msg.role,
                msg.content.len()
            );
            debug!("Message {} content: {:?}", i, msg.content);
        }

        let messages = Self::convert_messages(&request.messages)?;

        let mut converse_builder = self
            .client
            .converse()
            .model_id(&request.model)
            .set_messages(Some(messages));

        // Add system prompt with optional cache point
        if let Some(system_prompt) = &request.system_prompt {
            let system_blocks = self.build_system_blocks(system_prompt);
            for block in system_blocks {
                converse_builder = converse_builder.system(block);
            }
        }

        // Add tool configuration if tools are provided
        if let Some(tool_config) = Self::convert_tools(&request.tools) {
            converse_builder = converse_builder.tool_config(tool_config);
        }

        // Set inference configuration
        let mut inference_config = aws_sdk_bedrockruntime::types::InferenceConfiguration::builder()
            .max_tokens(request.max_tokens as i32);

        if let Some(temp) = request.temperature {
            inference_config = inference_config.temperature(temp);
        }

        converse_builder = converse_builder.inference_config(inference_config.build());

        let response = converse_builder.send().await.map_err(|e| {
            // Extract more detailed error information
            let err_str = format!("{:?}", e);
            let err_display = format!("{}", e);

            // Try to get raw response body for more details
            let raw_body = if let SdkError::ServiceError(ref service_err) = e {
                let raw = service_err.raw();
                let body_bytes = raw.body().bytes();
                body_bytes.map(|b| String::from_utf8_lossy(b).to_string())
            } else {
                None
            };

            debug!("AWS error (display): {}", err_display);
            debug!("AWS error (debug): {}", err_str);
            if let Some(ref body) = raw_body {
                debug!("AWS error (raw body): {}", body);
            }

            // Check for common error patterns
            if err_str.contains("ExpiredTokenException") || err_str.contains("expired") {
                BackendError::Authentication(
                    "AWS credentials expired. Run 'aws sso login' or refresh your credentials.".to_string()
                )
            } else if err_str.contains("AccessDeniedException") || err_str.contains("AccessDenied") {
                BackendError::Authentication(format!(
                    "Access denied. Ensure you have bedrock:InvokeModel permission for model '{}' in region '{}'.",
                    request.model, self.region
                ))
            } else if err_str.contains("ResourceNotFoundException") || err_str.contains("Could not resolve the foundation model") {
                BackendError::ModelNotAvailable(format!(
                    "Model '{}' not found in region '{}'. Check the model ID and ensure it's enabled in your AWS account.",
                    request.model, self.region
                ))
            } else if err_str.contains("ThrottlingException") {
                BackendError::RateLimited { retry_after_secs: Some(60) }
            } else if err_str.contains("ValidationException") {
                let body_info = raw_body.as_ref().map(|b| format!("\n\nRaw response: {}", b)).unwrap_or_default();
                BackendError::InvalidResponse(format!(
                    "Validation error for model '{}' in region '{}': {}{}",
                    request.model, self.region, e, body_info
                ))
            } else if err_str.contains("No credentials") || err_str.contains("failed to load credentials") {
                BackendError::Authentication(
                    "No AWS credentials found. Configure credentials via 'aws configure', environment variables, or SSO.".to_string()
                )
            } else {
                // Include full debug output for unknown errors
                BackendError::AwsSdk(format!("{}\n\nFull error: {:?}", e, e))
            }
        })?;

        // Extract stop reason
        let stop_reason = match response.stop_reason() {
            aws_sdk_bedrockruntime::types::StopReason::EndTurn => StopReason::EndTurn,
            aws_sdk_bedrockruntime::types::StopReason::ToolUse => StopReason::ToolUse,
            aws_sdk_bedrockruntime::types::StopReason::MaxTokens => StopReason::MaxTokens,
            aws_sdk_bedrockruntime::types::StopReason::StopSequence => StopReason::StopSequence,
            _ => StopReason::EndTurn,
        };

        // Extract usage including cache metrics
        let usage = response
            .usage()
            .map(|u| {
                let cache_read = u.cache_read_input_tokens().map(|v| v as i64);
                let cache_write = u.cache_write_input_tokens().map(|v| v as i64);

                debug!(
                    "Usage: input={}, output={}, cache_read={:?}, cache_write={:?}",
                    u.input_tokens(),
                    u.output_tokens(),
                    cache_read,
                    cache_write
                );

                Usage {
                    input_tokens: u.input_tokens() as i64,
                    output_tokens: u.output_tokens() as i64,
                    cache_read_input_tokens: cache_read,
                    cache_write_input_tokens: cache_write,
                }
            })
            .unwrap_or_default();

        // Extract content from response
        let content = response
            .output()
            .and_then(|o| o.as_message().ok())
            .map(|m| Self::convert_content(m.content()))
            .unwrap_or_default();

        // Get conversation_id from the first message
        let conversation_id = request
            .messages
            .first()
            .map(|m| m.conversation_id.clone())
            .unwrap_or_else(|| ConversationId::from("temp"));

        let message = Message::assistant(
            conversation_id,
            content,
            &request.model,
            usage.input_tokens,
            usage.output_tokens,
        );

        Ok(ChatResponse {
            message,
            stop_reason,
            usage,
        })
    }

    async fn send_message_stream(
        &self,
        request: &ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, BackendError>> + Send>>, BackendError>
    {
        info!(
            "Streaming message to Bedrock, model: {}, region: {}",
            request.model, self.region
        );
        debug!("Message count: {}", request.messages.len());
        debug!("Tools count: {}", request.tools.len());

        let messages = Self::convert_messages(&request.messages)?;

        let mut converse_builder = self
            .client
            .converse_stream()
            .model_id(&request.model)
            .set_messages(Some(messages));

        // Add system prompt with optional cache point
        if let Some(system_prompt) = &request.system_prompt {
            let system_blocks = self.build_system_blocks(system_prompt);
            for block in system_blocks {
                converse_builder = converse_builder.system(block);
            }
        }

        // Add tool configuration if tools are provided
        if let Some(tool_config) = Self::convert_tools(&request.tools) {
            converse_builder = converse_builder.tool_config(tool_config);
        }

        // Set inference configuration
        let mut inference_config = aws_sdk_bedrockruntime::types::InferenceConfiguration::builder()
            .max_tokens(request.max_tokens as i32);

        if let Some(temp) = request.temperature {
            inference_config = inference_config.temperature(temp);
        }

        converse_builder = converse_builder.inference_config(inference_config.build());

        // Send the streaming request and get the event stream
        let aws_response = converse_builder.send().await.map_err(|e| {
            // Use the same detailed error handling as non-streaming
            let err_str = format!("{:?}", e);

            if err_str.contains("ExpiredTokenException") || err_str.contains("expired") {
                BackendError::Authentication(
                    "AWS credentials expired. Run 'aws sso login' or refresh your credentials.".to_string()
                )
            } else if err_str.contains("AccessDeniedException") || err_str.contains("AccessDenied") {
                BackendError::Authentication(format!(
                    "Access denied. Ensure you have bedrock:InvokeModel permission for model '{}' in region '{}'.",
                    request.model, self.region
                ))
            } else if err_str.contains("ResourceNotFoundException") {
                BackendError::ModelNotAvailable(format!(
                    "Model '{}' not found in region '{}'. Check the model ID and ensure it's enabled in your AWS account.",
                    request.model, self.region
                ))
            } else if err_str.contains("ThrottlingException") {
                BackendError::RateLimited { retry_after_secs: Some(60) }
            } else {
                BackendError::AwsSdk(format!("{}", e))
            }
        })?;

        // Get the event stream from the response
        let mut aws_stream = aws_response.stream;

        // Convert AWS stream events to our StreamEvent type
        let stream = async_stream::stream! {
            loop {
                match aws_stream.recv().await {
                    Ok(Some(output)) => {
                        match Self::convert_stream_output(output) {
                            Ok(Some(event)) => yield Ok(event),
                            Ok(None) => {}, // Skip this event
                            Err(e) => {
                                yield Err(e);
                                break;
                            }
                        }
                    }
                    Ok(None) => {
                        // Stream ended normally
                        break;
                    }
                    Err(e) => {
                        warn!("Bedrock stream error: {}", e);
                        yield Err(BackendError::AwsSdk(format!("Stream error: {}", e)));
                        break;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    fn name(&self) -> &'static str {
        "AWS Bedrock"
    }

    async fn available_models(&self) -> Result<Vec<ModelInfo>, BackendError> {
        Ok(vec![]) // Models are region-dependent, use get_models() instead
    }

    fn default_model(&self) -> &ModelInfo {
        // Return Anthropic Sonnet 4 as the default model
        static DEFAULT_MODEL: LazyLock<ModelInfo> = LazyLock::new(|| ModelInfo {
            id: "eu.anthropic.claude-sonnet-4-5-20250929-v1:0".to_string(),
            name: "Claude Sonnet 4 (Bedrock)".to_string(),
            context_window: 200000,
            max_output_tokens: 16000,
            description: None,
            owner: None,
            pricing: None,
            modalities: None,
        });

        &DEFAULT_MODEL
    }
}

impl BedrockBackend {
    /// Convert AWS ConverseStreamOutput to our StreamEvent type
    /// Returns Ok(Some(event)) for events to emit, Ok(None) for events to skip
    fn convert_stream_output(
        output: ConverseStreamOutput,
    ) -> Result<Option<StreamEvent>, BackendError> {
        match output {
            ConverseStreamOutput::ContentBlockStart(event) => {
                let index = event.content_block_index() as usize;
                let start = event.start().ok_or_else(|| {
                    BackendError::InvalidResponse(
                        "ContentBlockStart missing start field".to_string(),
                    )
                })?;

                let block = match start {
                    BedrockContentBlockStart::ToolUse(tool_use) => {
                        ContentBlock::ToolUse {
                            id: tool_use.tool_use_id().to_string(),
                            name: tool_use.name().to_string(),
                            input: serde_json::Value::Object(serde_json::Map::new()), // Will be filled in by deltas
                        }
                    }
                    _ => {
                        // Text blocks don't have a start event in Bedrock, only deltas
                        // Start with empty text
                        ContentBlock::Text {
                            text: String::new(),
                        }
                    }
                };

                Ok(Some(StreamEvent::ContentBlockStart { index, block }))
            }
            ConverseStreamOutput::ContentBlockDelta(event) => {
                let index = event.content_block_index() as usize;
                let delta = event.delta().ok_or_else(|| {
                    BackendError::InvalidResponse(
                        "ContentBlockDelta missing delta field".to_string(),
                    )
                })?;

                let content_delta = match delta {
                    ContentBlockDelta::Text(text) => ContentDelta::Text {
                        text: text.to_string(),
                    },
                    ContentBlockDelta::ToolUse(tool_use) => ContentDelta::ToolInput {
                        partial_json: tool_use.input().to_string(),
                    },
                    _ => {
                        warn!("Unknown ContentBlockDelta type");
                        return Ok(None);
                    }
                };

                Ok(Some(StreamEvent::ContentBlockDelta {
                    index,
                    delta: content_delta,
                }))
            }
            ConverseStreamOutput::ContentBlockStop(event) => {
                let index = event.content_block_index() as usize;
                Ok(Some(StreamEvent::ContentBlockStop { index }))
            }
            ConverseStreamOutput::MessageStart(_) => {
                // Message start doesn't have useful info for us
                Ok(None)
            }
            ConverseStreamOutput::MessageStop(event) => {
                let stop_reason = match event.stop_reason() {
                    aws_sdk_bedrockruntime::types::StopReason::EndTurn => StopReason::EndTurn,
                    aws_sdk_bedrockruntime::types::StopReason::ToolUse => StopReason::ToolUse,
                    aws_sdk_bedrockruntime::types::StopReason::MaxTokens => StopReason::MaxTokens,
                    aws_sdk_bedrockruntime::types::StopReason::StopSequence => {
                        StopReason::StopSequence
                    }
                    _ => StopReason::EndTurn,
                };
                Ok(Some(StreamEvent::MessageStop { stop_reason }))
            }
            ConverseStreamOutput::Metadata(metadata) => {
                // Extract usage information including cache metrics
                if let Some(usage) = metadata.usage() {
                    let cache_read = usage.cache_read_input_tokens().map(|v| v as i64);
                    let cache_write = usage.cache_write_input_tokens().map(|v| v as i64);

                    debug!(
                        "Stream Usage: input={}, output={}, cache_read={:?}, cache_write={:?}",
                        usage.input_tokens(),
                        usage.output_tokens(),
                        cache_read,
                        cache_write
                    );

                    Ok(Some(StreamEvent::Usage(Usage {
                        input_tokens: usage.input_tokens() as i64,
                        output_tokens: usage.output_tokens() as i64,
                        cache_read_input_tokens: cache_read,
                        cache_write_input_tokens: cache_write,
                    })))
                } else {
                    Ok(None)
                }
            }
            _ => {
                // Unknown event type, skip
                warn!("Unknown ConverseStreamOutput variant");
                Ok(None)
            }
        }
    }

    /// Get available Anthropic models on Bedrock
    pub fn get_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "anthropic.claude-opus-4-20250514-v1:0".to_string(),
                name: "Claude Opus 4 (Bedrock)".to_string(),
                context_window: 200000,
                max_output_tokens: 32000,
                description: None,
                owner: None,
                pricing: None,
                modalities: None,
            },
            ModelInfo {
                id: "eu.anthropic.claude-sonnet-4-5-20250929-v1:0".to_string(),
                name: "Claude Sonnet 4 (Bedrock)".to_string(),
                context_window: 200000,
                max_output_tokens: 16000,
                description: None,
                owner: None,
                pricing: None,
                modalities: None,
            },
            ModelInfo {
                id: "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
                name: "Claude 3.5 Haiku (Bedrock)".to_string(),
                context_window: 200000,
                max_output_tokens: 8192,
                description: None,
                owner: None,
                pricing: None,
                modalities: None,
            },
            ModelInfo {
                id: "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
                name: "Claude 3.5 Haiku (Bedrock)".to_string(),
                context_window: 200000,
                max_output_tokens: 8192,
                description: None,
                owner: None,
                pricing: None,
                modalities: None,
            },
        ]
    }

    /// Get the region this backend is configured for
    pub fn region(&self) -> &str {
        &self.region
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_system_blocks_with_caching() {
        // Create a backend with caching enabled
        let backend = BedrockBackend {
            client: Client::from_conf(
                aws_sdk_bedrockruntime::Config::builder()
                    .behavior_version(aws_config::BehaviorVersion::latest())
                    .build(),
            ),
            region: "us-east-1".to_string(),
            enable_prompt_caching: true,
        };

        let blocks = backend.build_system_blocks("You are a helpful assistant");

        // Should have 2 blocks: Text and CachePoint
        assert_eq!(blocks.len(), 2);
        assert!(matches!(blocks[0], SystemContentBlock::Text(_)));
        assert!(matches!(blocks[1], SystemContentBlock::CachePoint(_)));
    }

    #[test]
    fn test_build_system_blocks_without_caching() {
        // Create a backend with caching disabled
        let backend = BedrockBackend {
            client: Client::from_conf(
                aws_sdk_bedrockruntime::Config::builder()
                    .behavior_version(aws_config::BehaviorVersion::latest())
                    .build(),
            ),
            region: "us-east-1".to_string(),
            enable_prompt_caching: false,
        };

        let blocks = backend.build_system_blocks("You are a helpful assistant");

        // Should have 1 block: Text only
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0], SystemContentBlock::Text(_)));
    }

    #[test]
    fn test_build_system_blocks_empty_prompt() {
        // Create a backend with caching enabled
        let backend = BedrockBackend {
            client: Client::from_conf(
                aws_sdk_bedrockruntime::Config::builder()
                    .behavior_version(aws_config::BehaviorVersion::latest())
                    .build(),
            ),
            region: "us-east-1".to_string(),
            enable_prompt_caching: true,
        };

        let blocks = backend.build_system_blocks("");

        // Empty prompt should return empty vec
        assert_eq!(blocks.len(), 0);
    }

    #[test]
    fn test_usage_with_cache_metrics() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: Some(90),
            cache_write_input_tokens: None,
        };

        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.cache_read_input_tokens, Some(90));
        assert_eq!(usage.cache_write_input_tokens, None);
    }

    #[test]
    fn test_usage_default() {
        let usage = Usage::default();

        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.cache_read_input_tokens, None);
        assert_eq!(usage.cache_write_input_tokens, None);
    }
}
