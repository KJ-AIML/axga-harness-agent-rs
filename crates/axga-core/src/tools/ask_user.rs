//! AskUserQuestion tool — prompts the user for answers interactively.
//!
//! The tool takes an array of questions and returns a structured JSON object
//! with the questions. When executed, the result includes a special marker
//! that the TUI detects to show an interactive question dialog.
//!
//! Questions support single-select and multi-select options, each with
//! a label and description.

use super::Tool;
use axga_shared::error::{AxgaError, AxgaResult};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

pub struct AskUserQuestionTool;

impl Tool for AskUserQuestionTool {
    fn name(&self) -> &str {
        "ask_user_question"
    }

    fn description(&self) -> &str {
        "Ask the user one or more questions interactively. \
         Use when you need user input to decide next steps or clarify \
         ambiguous requirements. Each question can have multiple choice \
         options or allow free-text answers."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "description": "Array of questions to ask the user.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "The question text."
                            },
                            "header": {
                                "type": "string",
                                "description": "A short label/header for this question."
                            },
                            "options": {
                                "type": "array",
                                "description": "Multiple choice options. Omit for free-text input.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "Display label for this option."
                                        },
                                        "description": {
                                            "type": "string",
                                            "description": "Explanation of what this option means."
                                        }
                                    },
                                    "required": ["label", "description"]
                                }
                            },
                            "multi_select": {
                                "type": "boolean",
                                "description": "Whether multiple options can be selected. Default: false."
                            }
                        },
                        "required": ["question", "header"]
                    }
                }
            },
            "required": ["questions"]
        })
    }

    fn execute(
        &self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let questions = input["questions"].as_array().ok_or_else(|| {
                AxgaError::ToolError {
                    tool: "ask_user_question".into(),
                    message: "missing 'questions' array".into(),
                }
            })?;

            if questions.is_empty() {
                return Err(AxgaError::ToolError {
                    tool: "ask_user_question".into(),
                    message: "questions array is empty".into(),
                });
            }

            // Return questions as the tool result. The TUI detects the
            // __axga_ask_user marker prefix and shows an interactive dialog.
            let output = serde_json::json!({
                "__axga_ask_user": true,
                "questions": questions
            });

            Ok(format!("__AXGA_ASK_USER__{output}"))
        })
    }
}

/// Check if a tool result string is an AskUserQuestion response.
pub fn parse_ask_user_result(result: &str) -> Option<Value> {
    let marker = "__AXGA_ASK_USER__";
    result.strip_prefix(marker).and_then(|json_str| {
        let parsed: Value = serde_json::from_str(json_str).ok()?;
        if parsed.get("__axga_ask_user").and_then(|v| v.as_bool()) == Some(true) {
            Some(parsed)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ask_user_question_tool_name() {
        let tool = AskUserQuestionTool;
        assert_eq!(tool.name(), "ask_user_question");
    }

    #[test]
    fn ask_user_question_tool_description() {
        let tool = AskUserQuestionTool;
        assert!(tool.description().contains("interactively"));
    }

    #[test]
    fn ask_user_question_requires_questions() {
        let tool = AskUserQuestionTool;
        let params = tool.parameters();
        let req = params["required"].as_array().unwrap();
        assert!(req.contains(&serde_json::Value::String("questions".into())));
    }

    #[test]
    fn parse_ask_user_result_valid() {
        let result = r#"__AXGA_ASK_USER__{"__axga_ask_user":true,"questions":[{"question":"Test?","header":"Test"}]}"#;
        let parsed = parse_ask_user_result(result);
        assert!(parsed.is_some());
        let p = parsed.unwrap();
        assert_eq!(p["__axga_ask_user"], true);
        assert_eq!(p["questions"][0]["question"], "Test?");
    }

    #[test]
    fn parse_ask_user_result_not_marked() {
        let result = "just some normal output";
        assert!(parse_ask_user_result(result).is_none());
    }
}
