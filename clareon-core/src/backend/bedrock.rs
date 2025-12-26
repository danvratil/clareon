//! AWS Bedrock backend implementation

use std::pin::Pin;

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::error::SdkError;
use aws_sdk_bedrockruntime::types::{
    ContentBlock as BedrockContentBlock, ConversationRole, Message as BedrockMessage,
    SystemContentBlock, ToolResultBlock, ToolResultContentBlock, ToolUseBlock,
};
use aws_sdk_bedrockruntime::Client;
use aws_smithy_types::Document;
use futures::Stream;
use tracing::{debug, info};

use super::traits::{
    ChatRequest, ChatResponse, LlmBackend, ModelInfo, StopReason, StreamEvent, Usage,
};
use crate::error::BackendError;
use crate::types::{ContentBlock, Message, Role, ToolResultContent};

/// AWS Bedrock backend for Claude models
pub struct BedrockBackend {
    client: Client,
    region: String,
}

impl BedrockBackend {
    /// Create a new Bedrock backend using the default credential chain
    pub async fn new(region: impl Into<String>) -> Result<Self, BackendError> {
        let region = region.into();
        info!("Initializing Bedrock backend in region: {}", region);

        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_config::Region::new(region.clone()))
            .load()
            .await;

        let client = Client::new(&config);

        Ok(Self { client, region })
    }

    /// Create a new Bedrock backend with a specific AWS profile
    pub async fn with_profile(
        region: impl Into<String>,
        profile: impl Into<String>,
    ) -> Result<Self, BackendError> {
        let region = region.into();
        let profile = profile.into();
        info!(
            "Initializing Bedrock backend in region: {} with profile: {}",
            region, profile
        );

        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_config::Region::new(region.clone()))
            .profile_name(&profile)
            .load()
            .await;

        let client = Client::new(&config);

        Ok(Self { client, region })
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

                Ok(BedrockMessage::builder()
                    .role(role)
                    .set_content(Some(content))
                    .build()
                    .map_err(|e| BackendError::InvalidResponse(e.to_string()))?)
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
        }

        let messages = Self::convert_messages(&request.messages)?;

        let mut converse_builder = self
            .client
            .converse()
            .model_id(&request.model)
            .set_messages(Some(messages));

        // Add system prompt if provided and not empty
        if let Some(system_prompt) = &request.system_prompt {
            if !system_prompt.is_empty() {
                converse_builder =
                    converse_builder.system(SystemContentBlock::Text(system_prompt.clone()));
            }
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

        // Extract usage
        let usage = response
            .usage()
            .map(|u| Usage {
                input_tokens: u.input_tokens() as i64,
                output_tokens: u.output_tokens() as i64,
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
            .map(|m| m.conversation_id)
            .unwrap_or(0);

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
        _request: &ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, BackendError>> + Send>>, BackendError>
    {
        // TODO: Implement streaming using ConverseStream
        Err(BackendError::InvalidResponse(
            "Streaming not yet implemented for Bedrock backend".to_string(),
        ))
    }

    fn name(&self) -> &'static str {
        "AWS Bedrock"
    }

    fn available_models(&self) -> &[ModelInfo] {
        &[] // Models are region-dependent, use get_models() instead
    }
}

impl BedrockBackend {
    /// Get available Claude models on Bedrock
    pub fn get_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "anthropic.claude-opus-4-20250514-v1:0".to_string(),
                name: "Claude Opus 4 (Bedrock)".to_string(),
                context_window: 200000,
                max_output_tokens: 32000,
            },
            ModelInfo {
                id: "anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
                name: "Claude Sonnet 4 (Bedrock)".to_string(),
                context_window: 200000,
                max_output_tokens: 16000,
            },
            ModelInfo {
                id: "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
                name: "Claude 3.5 Haiku (Bedrock)".to_string(),
                context_window: 200000,
                max_output_tokens: 8192,
            },
            ModelInfo {
                id: "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
                name: "Claude 3 Haiku (Bedrock)".to_string(),
                context_window: 200000,
                max_output_tokens: 4096,
            },
        ]
    }

    /// Get the region this backend is configured for
    pub fn region(&self) -> &str {
        &self.region
    }
}
