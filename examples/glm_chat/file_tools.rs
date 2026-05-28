use super::Tool;
use serde_json::{Value, json};
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

pub(super) const FILE_TOOL_ENABLE_ENV: &str = "GLM_CHAT_ENABLE_FILE_TOOLS";
const FILE_TOOL_ROOT_ENV: &str = "GLM_CHAT_FILE_ROOT";
const FILE_TOOL_ALLOW_ABSOLUTE_ENV: &str = "GLM_CHAT_ALLOW_ABSOLUTE_PATHS";
const FILE_TOOL_MAX_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug)]
pub(super) struct ToolSandbox {
    pub(super) root: PathBuf,
    max_bytes: usize,
    allow_absolute_paths: bool,
}

impl ToolSandbox {
    pub(super) fn from_env() -> Result<Option<Self>, String> {
        if env::var(FILE_TOOL_ENABLE_ENV).as_deref() != Ok("1") {
            return Ok(None);
        }

        let root = env::var(FILE_TOOL_ROOT_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let allow_absolute_paths = env::var(FILE_TOOL_ALLOW_ABSOLUTE_ENV).as_deref() == Ok("1");
        Self::new(root, FILE_TOOL_MAX_BYTES, allow_absolute_paths).map(Some)
    }

    fn new(root: PathBuf, max_bytes: usize, allow_absolute_paths: bool) -> Result<Self, String> {
        let root = fs::canonicalize(&root)
            .map_err(|e| format!("invalid sandbox root {}: {}", root.display(), e))?;
        if !root.is_dir() {
            return Err(format!(
                "sandbox root is not a directory: {}",
                root.display()
            ));
        }

        Ok(Self {
            root,
            max_bytes,
            allow_absolute_paths,
        })
    }

    fn resolve(&self, requested: &str) -> Result<PathBuf, String> {
        let requested = requested.trim();
        let requested = if requested.is_empty() { "." } else { requested };
        let path = Path::new(requested);

        if path.is_absolute() && !self.allow_absolute_paths {
            return Err("absolute paths are disabled for GLM file tools".to_string());
        }

        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        let canonical = fs::canonicalize(&candidate)
            .map_err(|e| format!("cannot access {}: {}", requested, e))?;

        if !canonical.starts_with(&self.root) {
            return Err(format!(
                "path must stay inside sandbox root {}",
                self.root.display()
            ));
        }

        Ok(canonical)
    }

    fn display_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .display()
            .to_string()
    }
}

pub(super) fn get_tools(file_tools_enabled: bool) -> Vec<Tool> {
    if !file_tools_enabled {
        return Vec::new();
    }

    vec![
        Tool {
            name: "read_file".to_string(),
            description: "Read a file inside the configured GLM_CHAT_FILE_ROOT sandbox".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Sandbox-relative file path"
                    }
                },
                "required": ["path"]
            }),
        },
        Tool {
            name: "list_files".to_string(),
            description: "List files inside the configured GLM_CHAT_FILE_ROOT sandbox".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Sandbox-relative directory path"
                    }
                },
                "required": ["path"]
            }),
        },
        Tool {
            name: "search_files".to_string(),
            description: "Search file names inside the configured GLM_CHAT_FILE_ROOT sandbox"
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "File-name substring, or * to list matches"
                    }
                },
                "required": ["pattern"]
            }),
        },
    ]
}

pub(super) fn execute_tool(name: &str, input: &Value, sandbox: Option<&ToolSandbox>) -> String {
    let Some(sandbox) = sandbox else {
        return "Error: local file tools are disabled; set GLM_CHAT_ENABLE_FILE_TOOLS=1 to enable them".to_string();
    };

    match name {
        "read_file" => {
            let path = input["path"].as_str().unwrap_or("");
            read_sandboxed_file(sandbox, path)
        }
        "list_files" => {
            let path = input["path"].as_str().unwrap_or(".");
            list_sandboxed_files(sandbox, path)
        }
        "search_files" => {
            let pattern = input["pattern"].as_str().unwrap_or("*");
            let mut results = Vec::new();
            search_recursive(&sandbox.root, sandbox, pattern, &mut results, 0, 3);
            if results.is_empty() {
                "No files found".to_string()
            } else {
                results.join("\n")
            }
        }
        _ => format!("Unknown tool: {}", name),
    }
}

fn read_sandboxed_file(sandbox: &ToolSandbox, path: &str) -> String {
    let path = match sandbox.resolve(path) {
        Ok(path) => path,
        Err(e) => return format!("Error: {}", e),
    };
    if !path.is_file() {
        return format!("Error: not a file: {}", sandbox.display_path(&path));
    }

    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(e) => return format!("Error: {}", e),
    };
    let mut buffer = Vec::with_capacity(sandbox.max_bytes.min(4096));
    let limit = sandbox.max_bytes.saturating_add(1) as u64;
    if let Err(e) = Read::by_ref(&mut file).take(limit).read_to_end(&mut buffer) {
        return format!("Error: {}", e);
    }
    let truncated = buffer.len() > sandbox.max_bytes;
    if truncated {
        buffer.truncate(sandbox.max_bytes);
    }

    let content = String::from_utf8_lossy(&buffer);
    let lines: Vec<&str> = content.lines().take(100).collect();
    let truncation_note = if truncated {
        format!(
            "\n\n[truncated at {} bytes by GLM file tool sandbox]",
            sandbox.max_bytes
        )
    } else {
        String::new()
    };

    format!(
        "Read {} lines from {}\n\n{}{}",
        lines.len(),
        sandbox.display_path(&path),
        lines.join("\n"),
        truncation_note
    )
}

fn list_sandboxed_files(sandbox: &ToolSandbox, path: &str) -> String {
    let path = match sandbox.resolve(path) {
        Ok(path) => path,
        Err(e) => return format!("Error: {}", e),
    };
    if !path.is_dir() {
        return format!("Error: not a directory: {}", sandbox.display_path(&path));
    }

    match fs::read_dir(&path) {
        Ok(entries) => {
            let mut files: Vec<String> = entries
                .filter_map(|e| e.ok())
                .take(200)
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    let is_real_dir = e
                        .file_type()
                        .map(|file_type| file_type.is_dir() && !file_type.is_symlink())
                        .unwrap_or(false);
                    if is_real_dir {
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

fn search_recursive(
    dir: &Path,
    sandbox: &ToolSandbox,
    pattern: &str,
    results: &mut Vec<String>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if name.contains(pattern) || pattern == "*" {
                results.push(sandbox.display_path(&path));
            }

            let is_real_dir = entry
                .file_type()
                .map(|file_type| file_type.is_dir() && !file_type.is_symlink())
                .unwrap_or(false);
            if is_real_dir && !name.starts_with('.') {
                search_recursive(&path, sandbox, pattern, results, depth + 1, max_depth);
            }

            if results.len() >= 20 {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Display;
    use std::io;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempSandbox {
        root: PathBuf,
        parent: PathBuf,
    }

    impl TempSandbox {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or_else(
                    |e| panic!("system clock is before UNIX_EPOCH: {}", e),
                    |d| d,
                )
                .as_nanos();
            let parent = env::temp_dir().join(format!(
                "rui-glm-chat-test-{}-{}",
                std::process::id(),
                nonce
            ));
            let root = parent.join("root");
            must(fs::create_dir_all(&root), "create test sandbox root");
            Self { root, parent }
        }

        fn sandbox(&self, max_bytes: usize, allow_absolute_paths: bool) -> ToolSandbox {
            must(
                ToolSandbox::new(self.root.clone(), max_bytes, allow_absolute_paths),
                "create tool sandbox",
            )
        }
    }

    impl Drop for TempSandbox {
        fn drop(&mut self) {
            if let Err(e) = fs::remove_dir_all(&self.parent) {
                if e.kind() != io::ErrorKind::NotFound {
                    eprintln!(
                        "failed to remove test sandbox {}: {}",
                        self.parent.display(),
                        e
                    );
                }
            }
        }
    }

    fn must<T, E: Display>(result: Result<T, E>, action: &str) -> T {
        match result {
            Ok(value) => value,
            Err(e) => panic!("{}: {}", action, e),
        }
    }

    #[test]
    fn read_file_rejects_parent_traversal() {
        let temp = TempSandbox::new();
        must(
            fs::write(temp.root.join("allowed.txt"), "allowed"),
            "write allowed file",
        );
        must(
            fs::write(temp.parent.join("secret.txt"), "secret"),
            "write secret file",
        );
        let sandbox = temp.sandbox(1024, false);

        let allowed = execute_tool("read_file", &json!({"path": "allowed.txt"}), Some(&sandbox));
        assert!(allowed.contains("allowed"));

        let denied = execute_tool(
            "read_file",
            &json!({"path": "../secret.txt"}),
            Some(&sandbox),
        );
        assert!(denied.contains("path must stay inside sandbox root"));
        assert!(!denied.contains("secret"));
    }

    #[test]
    fn absolute_paths_require_explicit_opt_in() {
        let temp = TempSandbox::new();
        let file = temp.root.join("allowed.txt");
        must(fs::write(&file, "allowed"), "write allowed file");
        let sandbox = temp.sandbox(1024, false);

        let result = execute_tool(
            "read_file",
            &json!({"path": file.display().to_string()}),
            Some(&sandbox),
        );

        assert!(result.contains("absolute paths are disabled"));
        assert!(!result.contains("allowed"));
    }

    #[test]
    fn read_file_caps_returned_bytes() {
        let temp = TempSandbox::new();
        must(
            fs::write(temp.root.join("large.txt"), "abcdef"),
            "write large file",
        );
        let sandbox = temp.sandbox(4, false);

        let result = execute_tool("read_file", &json!({"path": "large.txt"}), Some(&sandbox));

        assert!(result.contains("abcd"));
        assert!(!result.contains("abcdef"));
        assert!(result.contains("truncated at 4 bytes"));
    }
}
