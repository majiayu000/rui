//! GLM CLI Chat Demo with Tool Use
//!
//! 使用方法: GLM_API_KEY=your_key cargo run --example glm_chat

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::fs;
use std::path::Path;
use unicode_width::UnicodeWidthStr;

const API_URL: &str = "https://open.bigmodel.cn/api/anthropic/v1/messages";

// ANSI 颜色和样式
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const MAGENTA: &str = "\x1b[35m";
const RED: &str = "\x1b[31m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";
const CLEAR_LINE: &str = "\x1b[2K\r";

// Unicode 框线字符
const BOX_TL: &str = "╭";
const BOX_TR: &str = "╮";
const BOX_BL: &str = "╰";
const BOX_BR: &str = "╯";
const BOX_H: &str = "─";
const BOX_V: &str = "│";
const BULLET: &str = "⏺";
const ARROW: &str = "⎿";

#[derive(Serialize, Clone)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<MessageParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Tool>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct MessageParam {
    role: String,
    content: MessageContent,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Serialize, Clone)]
struct Tool {
    name: String,
    description: String,
    input_schema: Value,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    content: Vec<ResponseBlock>,
    stop_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum ResponseBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
}

fn get_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "read_file".to_string(),
            description: "读取指定路径的文件内容".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件路径"
                    }
                },
                "required": ["path"]
            }),
        },
        Tool {
            name: "list_files".to_string(),
            description: "列出指定目录下的文件和文件夹".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "目录路径"
                    }
                },
                "required": ["path"]
            }),
        },
        Tool {
            name: "search_files".to_string(),
            description: "在当前目录搜索匹配的文件名".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "搜索模式（支持通配符）"
                    }
                },
                "required": ["pattern"]
            }),
        },
    ]
}

fn execute_tool(name: &str, input: &Value) -> String {
    match name {
        "read_file" => {
            let path = input["path"].as_str().unwrap_or("");
            match fs::read_to_string(path) {
                Ok(content) => {
                    let lines: Vec<&str> = content.lines().take(100).collect();
                    format!("Read {} lines\n\n{}", lines.len(), lines.join("\n"))
                }
                Err(e) => format!("Error: {}", e),
            }
        }
        "list_files" => {
            let path = input["path"].as_str().unwrap_or(".");
            match fs::read_dir(path) {
                Ok(entries) => {
                    let mut files: Vec<String> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| {
                            let name = e.file_name().to_string_lossy().to_string();
                            if e.path().is_dir() {
                                format!("{}/", name)
                            } else {
                                name
                            }
                        })
                        .collect();
                    files.sort();
                    files.join("\n")
                }
                Err(e) => format!("Error: {}", e),
            }
        }
        "search_files" => {
            let pattern = input["pattern"].as_str().unwrap_or("*");
            let mut results = Vec::new();
            search_recursive(Path::new("."), pattern, &mut results, 0, 3);
            if results.is_empty() {
                "No files found".to_string()
            } else {
                results.join("\n")
            }
        }
        _ => format!("Unknown tool: {}", name),
    }
}

fn search_recursive(dir: &Path, pattern: &str, results: &mut Vec<String>, depth: usize, max_depth: usize) {
    if depth > max_depth {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if name.contains(pattern) || pattern == "*" {
                results.push(path.display().to_string());
            }

            if path.is_dir() && !name.starts_with('.') {
                search_recursive(&path, pattern, results, depth + 1, max_depth);
            }

            if results.len() >= 20 {
                return;
            }
        }
    }
}

fn print_banner() {
    const BOX_WIDTH: usize = 45;

    println!();
    println!("{}{}╭{}╮{}", BOLD, CYAN, "─".repeat(BOX_WIDTH), RESET);

    // 第一行：标题
    let title = "GLM Chat CLI";
    let subtitle = "with Tool Use";
    let line1 = format!("  {}{}{}  {}{}{}", BOLD, title, RESET, DIM, subtitle, RESET);
    let line1_width = 2 + title.width() + 2 + subtitle.width();
    let pad1 = BOX_WIDTH - line1_width;
    println!("{}{}│{}{}{}│{}", BOLD, CYAN, RESET, line1, " ".repeat(pad1), CYAN, );
    println!("{}│{}", CYAN, RESET);

    // 第二行：提示
    let hint = "输入 'quit' 退出 | 'clear' 清屏";
    let line2 = format!("  {}{}{}", DIM, hint, RESET);
    let line2_width = 2 + hint.width();
    let pad2 = BOX_WIDTH - line2_width;
    println!("{}{}│{}{}{}│{}", BOLD, CYAN, RESET, line2, " ".repeat(pad2), CYAN);

    println!("{}{}╰{}╯{}", BOLD, CYAN, "─".repeat(BOX_WIDTH), RESET);
    println!();
}

fn print_tool_call(name: &str, input: &Value) {
    let input_str = if let Some(obj) = input.as_object() {
        obj.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        input.to_string()
    };

    println!();
    println!("{}{} {}{}({}){}",MAGENTA, BULLET, BOLD, name, RESET, RESET);
}

fn print_tool_result(result: &str) {
    let lines: Vec<&str> = result.lines().take(5).collect();
    let preview = lines.join("\n  {}  ");
    println!("  {}{} {}{}", MAGENTA, ARROW, DIM, RESET);

    for line in result.lines().take(3) {
        println!("  {}{}  {}{}", MAGENTA, ARROW, line, RESET);
    }

    if result.lines().count() > 3 {
        println!("  {}{}  {}...({} more lines){}", MAGENTA, ARROW, DIM, result.lines().count() - 3, RESET);
    }
}

fn print_thinking(text: &str) {
    println!();
    println!("{}{}┌─ Thinking ─────────────────────────────{}", DIM, YELLOW, RESET);
    for line in text.lines().take(5) {
        println!("{}{}│ {}{}", DIM, YELLOW, line, RESET);
    }
    if text.lines().count() > 5 {
        println!("{}{}│ ...{}", DIM, YELLOW, RESET);
    }
    println!("{}{}└─────────────────────────────────────────{}", DIM, YELLOW, RESET);
}

struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Spinner {
    fn new(message: &str) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let message = message.to_string();

        let handle = std::thread::spawn(move || {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0;

            while running_clone.load(Ordering::Relaxed) {
                print!("{}{}{} {}{}", CLEAR_LINE, YELLOW, frames[i], message, RESET);
                io::stdout().flush().unwrap();
                i = (i + 1) % frames.len();
                std::thread::sleep(Duration::from_millis(80));
            }
            print!("{}", CLEAR_LINE);
            io::stdout().flush().unwrap();
        });

        Self {
            running,
            handle: Some(handle),
        }
    }

    fn stop(mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

async fn send_request(
    client: &Client,
    api_key: &str,
    messages: &[MessageParam],
    tools: &[Tool],
) -> Result<ChatResponse, Box<dyn std::error::Error>> {
    let request = ChatRequest {
        model: "claude-3-5-sonnet-20241022".to_string(),
        max_tokens: 8192,
        messages: messages.to_vec(),
        tools: Some(tools.to_vec()),
    };

    let response = client
        .post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(format!("API Error: {}", error_text).into());
    }

    let chat_response: ChatResponse = response.json().await?;
    Ok(chat_response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("GLM_API_KEY")
        .expect("请设置环境变量 GLM_API_KEY");

    let client = Client::new();
    let mut messages: Vec<MessageParam> = Vec::new();
    let tools = get_tools();

    print_banner();

    loop {
        print!("{}{}❯{} ", BOLD, GREEN, RESET);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        match input.to_lowercase().as_str() {
            "quit" | "exit" => {
                println!("\n{}👋 再见！{}\n", DIM, RESET);
                break;
            }
            "clear" => {
                print!("\x1b[2J\x1b[H");
                print_banner();
                continue;
            }
            "" => continue,
            _ => {}
        }

        messages.push(MessageParam {
            role: "user".to_string(),
            content: MessageContent::Text(input.to_string()),
        });

        // 处理可能的多轮工具调用
        loop {
            let spinner = Spinner::new("思考中...");
            let result = send_request(&client, &api_key, &messages, &tools).await;
            spinner.stop();

            match result {
                Ok(response) => {
                    let mut tool_uses = Vec::new();
                    let mut has_text = false;

                    // 处理响应
                    for block in &response.content {
                        match block {
                            ResponseBlock::Thinking { thinking } => {
                                print_thinking(thinking);
                            }
                            ResponseBlock::Text { text } => {
                                if !text.is_empty() {
                                    println!();
                                    println!("{}", text);
                                    has_text = true;
                                }
                            }
                            ResponseBlock::ToolUse { id, name, input } => {
                                print_tool_call(name, input);

                                // 执行工具
                                let tool_result = execute_tool(name, input);
                                print_tool_result(&tool_result);

                                tool_uses.push((id.clone(), tool_result));
                            }
                        }
                    }

                    // 保存助手消息
                    let assistant_content: Vec<ContentBlock> = response.content.iter().filter_map(|b| {
                        match b {
                            ResponseBlock::Text { text } => Some(ContentBlock::Text { text: text.clone() }),
                            ResponseBlock::ToolUse { id, name, input } => Some(ContentBlock::ToolUse {
                                id: id.clone(),
                                name: name.clone(),
                                input: input.clone(),
                            }),
                            _ => None,
                        }
                    }).collect();

                    messages.push(MessageParam {
                        role: "assistant".to_string(),
                        content: MessageContent::Blocks(assistant_content),
                    });

                    // 如果有工具调用，添加结果并继续
                    if !tool_uses.is_empty() {
                        let tool_results: Vec<ContentBlock> = tool_uses
                            .into_iter()
                            .map(|(id, result)| ContentBlock::ToolResult {
                                tool_use_id: id,
                                content: result,
                            })
                            .collect();

                        messages.push(MessageParam {
                            role: "user".to_string(),
                            content: MessageContent::Blocks(tool_results),
                        });

                        // 继续对话让模型处理工具结果
                        continue;
                    }

                    println!();
                    break;
                }
                Err(e) => {
                    println!("\n{}[错误] {}{}\n", RED, e, RESET);
                    messages.pop();
                    break;
                }
            }
        }
    }

    Ok(())
}
