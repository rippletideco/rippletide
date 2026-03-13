use std::fs;
use std::io;
use std::io::Write;
use std::path::PathBuf;

use clap::Parser;
use directories::ProjectDirs;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "rippletide", about = "Rippletide CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Connect and configure the project files
    Connect {
        /// Reserved for backward compatibility
        #[arg(long)]
        read_only: bool,
        /// Use an existing Rippletide agent id (skips account onboarding)
        #[arg(long)]
        agent_id: Option<String>,
    },
    /// Log out and remove stored credentials
    Logout,
}

const LOCAL_API_URL: &str = "http://localhost:3000";

const PROD_API_URL: &str = "https://dashboard-rippletide.up.railway.app/coding-agent";
const DEFAULT_MCP_URL: &str = "https://mcp.rippletide.com/mcp";

const SIGN_UP_PATH: &str = "/api/auth/sign-up/email";
const UPLOAD_URL: &str = "https://coding-agent.up.railway.app/upload";

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Environment {
    Local,
    Production,
}

impl Default for Environment {
    fn default() -> Self {
        Self::Production
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Production => write!(f, "production"),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct Config {
    user_id: Option<String>,
    session_token: Option<String>,
    agent_id: Option<String>,
    email: Option<String>,
    api_url: Option<String>,
    #[serde(default)]
    environment: Environment,
}

impl Config {
    fn api_url(&self) -> &str {
        if let Some(ref custom) = self.api_url {
            return custom.as_str();
        }
        match self.environment {
            Environment::Local => LOCAL_API_URL,
            Environment::Production => PROD_API_URL,
        }
    }
}

fn config_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "Rippletide", "Rippletide")
        .map(|dirs| dirs.config_dir().join("config.json"))
}

fn load_config() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
    };
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config(config: &Config) -> io::Result<()> {
    let Some(path) = config_path() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "cannot determine config directory",
        ));
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    fs::write(&path, json)
}

// --- Email auth ---

fn prompt(label: &str) -> io::Result<String> {
    print!("  {label}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

#[derive(Deserialize)]
struct SignUpUser {
    id: String,
    email: String,
}

#[derive(Deserialize)]
struct SignUpResponse {
    token: String,
    user: SignUpUser,
}

fn sign_up(api_url: &str, name: &str, email: &str, password: &str) -> Result<SignUpResponse, String> {
    let url = format!("{}{}", api_url, SIGN_UP_PATH);
    let resp = ureq::post(&url)
        .send_json(serde_json::json!({
            "name": name,
            "email": email,
            "password": password,
        }))
        .map_err(|e| format!("Network error: {e}"))?;
    let sign_up_resp: SignUpResponse = resp
        .into_json()
        .map_err(|e| format!("Invalid response: {e}"))?;
    Ok(sign_up_resp)
}

// --- CWD bootstrap helpers ---

const AGENT_INSTRUCTIONS: &str = r#"# Hook-First Planning Instructions

## Objective

Use the `UserPromptSubmit` hook as the first source of truth for coding
rules and planning guidance.

These instructions apply to both `AGENTS.md` and `CLAUDE.md`.

## Hook-First Rule

Before answering any request that may lead to planning, code generation,
refactoring, architecture, or tests, you MUST first use the hook-injected
context tagged `[Coding Rules from Rippletide]` when it is present.

This requirement applies especially to plan mode requests such as:

- `/plan ...`
- requests that ask for a step-by-step implementation plan
- requests that ask what should be built before writing code

## Required Behavior Before Plan Mode

When the user enters a plan-style request, the assistant must treat the
hook result as input that is processed before producing the plan.

If hook rules are present:

1. Begin the response by explicitly naming the rules being applied.
2. Make the plan consistent with those rules.
3. Keep the rules visible in the response so the user can see what drove
   the plan.

Use a direct format such as:

`Applying rules: Rule A, Rule B, Rule C`

If the hook returns no rules, say so explicitly before continuing:

`Applying rules: none returned by hook`

## Required Behavior Before Code Generation

Before generating code, examples, patches, refactors, or tests:

1. Read the hook-injected rules first.
2. State which rules are being applied.
3. Ensure the implementation follows those rules.
4. If relevant, explain which rule changed the implementation or plan.

## Query Source

The hook query should use the user's current request text, not a fixed
prompt. For example, if the user submits:

`/plan write a hello world`

then the hook query should contain that exact text as the request being
evaluated.

## Enforcement

Do not produce planning or code output silently.

Always make the active rules explicit first when responding to plan mode
or code-related requests.
"#;

fn build_agent_instructions() -> String {
    AGENT_INSTRUCTIONS.to_string()
}

fn ensure_agent_files() -> io::Result<u8> {
    let cwd = std::env::current_dir()?;
    let content = build_agent_instructions();
    for name in ["AGENTS.md", "CLAUDE.md"] {
        let path = cwd.join(name);
        fs::write(&path, &content)?;
    }
    Ok(2)
}

fn mcp_agent_endpoint(agent_id: &str) -> String {
    format!("{DEFAULT_MCP_URL}?agentId={agent_id}")
}

fn ensure_json_mcp_config(path: &PathBuf, server_name: &str, server_value: serde_json::Value) -> io::Result<bool> {
    let mut config = if path.exists() {
        let existing = fs::read_to_string(path).unwrap_or_else(|_| "{}".to_string());
        serde_json::from_str(&existing).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let original = config.clone();
    if config.as_object_mut().is_none() {
        config = serde_json::json!({});
    }

    let root_obj = config.as_object_mut().unwrap();
    let servers = root_obj
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));
    if !servers.is_object() {
        *servers = serde_json::json!({});
    }

    let servers_obj = servers.as_object_mut().unwrap();
    servers_obj.insert(server_name.to_string(), server_value);

    let changed = config != original;
    if changed {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(&config)?)?;
    }

    Ok(changed)
}

fn ensure_dot_mcp_config(agent_id: &str) -> io::Result<bool> {
    let cwd = std::env::current_dir()?;
    let path = cwd.join(".mcp.json");
    let endpoint = mcp_agent_endpoint(agent_id);
    let server_value = serde_json::json!({
        "type": "http",
        "url": endpoint
    });
    ensure_json_mcp_config(&path, "rippletide", server_value)
}

fn ensure_codex_config(agent_id: &str) -> io::Result<bool> {
    let cwd = std::env::current_dir()?;
    let dir = cwd.join(".codex");
    let path = dir.join("config.toml");
    let endpoint = mcp_agent_endpoint(agent_id);

    let mut content = if path.exists() {
        let existing = fs::read_to_string(&path).unwrap_or_default();
        existing.parse::<toml::Value>().unwrap_or_else(|_| toml::Value::Table(toml::Table::new()))
    } else {
        toml::Value::Table(toml::Table::new())
    };

    if content.as_table_mut().is_none() {
        content = toml::Value::Table(toml::Table::new());
    }

    let original = content.clone();
    {
        let root = content.as_table_mut().unwrap();
        let servers = root
            .entry("mcp_servers".to_string())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if servers.as_table_mut().is_none() {
            *servers = toml::Value::Table(toml::Table::new());
        }
        let servers_table = servers.as_table_mut().unwrap();

        let mut server = toml::Table::new();
        server.insert("command".into(), toml::Value::String("npx".into()));
        server.insert(
            "args".into(),
            toml::Value::Array(vec![
                toml::Value::String("-y".into()),
                toml::Value::String("mcp-remote".into()),
                toml::Value::String(endpoint),
            ]),
        );
        servers_table.insert("rippletide".into(), toml::Value::Table(server));
    }

    let changed = content != original;
    if changed {
        fs::create_dir_all(&dir)?;
        fs::write(&path, content.to_string())?;
    }

    Ok(changed)
}

fn claude_desktop_config_path() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        let home = std::env::var("HOME").ok()?;
        return Some(PathBuf::from(home).join("Library/Application Support/Claude/claude_desktop_config.json"));
    }

    if cfg!(target_os = "windows") {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return Some(PathBuf::from(appdata).join("Claude\\claude_desktop_config.json"));
        }
        if let Ok(home) = std::env::var("USERPROFILE") {
            return Some(PathBuf::from(home).join("AppData\\Roaming\\Claude\\claude_desktop_config.json"));
        }
        return None;
    }

    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".config/Claude/claude_desktop_config.json"))
}

fn ensure_claude_desktop_config(agent_id: &str) -> io::Result<bool> {
    let path = match claude_desktop_config_path() {
        Some(p) => p,
        None => return Ok(false),
    };
    let endpoint = mcp_agent_endpoint(agent_id);
    let server_value = serde_json::json!({
        "command": "npx",
        "args": ["-y", "mcp-remote", endpoint]
    });
    ensure_json_mcp_config(&path, "rippletide", server_value)
}

const HOOK_SCRIPT: &str = r#"#!/bin/bash

# Read hook input from stdin
hook_input=$(cat)
if [[ -z "${hook_input//[[:space:]]/}" ]]; then
  exit 0
fi

# Read user_id from Rippletide config (macOS or Linux)
CONFIG_FILE="$HOME/Library/Application Support/com.Rippletide.Rippletide/config.json"
if [[ ! -f "$CONFIG_FILE" ]]; then
  CONFIG_FILE="$HOME/.config/Rippletide/Rippletide/config.json"
fi
if [[ ! -f "$CONFIG_FILE" ]]; then
  exit 0
fi

USER_ID=$(jq -r '.user_id // empty' "$CONFIG_FILE" 2>/dev/null)
if [[ -z "$USER_ID" ]]; then
  exit 0
fi

# Query coding rules for the current user request
PAYLOAD=$(jq -Rn \
  --arg query "$hook_input" \
  '{query: $query, beam_width: 2, beam_max_depth: 8}' 2>/dev/null)
if [[ -z "$PAYLOAD" ]]; then
  exit 0
fi

RESPONSE=$(curl -s --max-time 180 -X POST "https://coding-agent.up.railway.app/query-rules" \
  -H "Content-Type: application/json" \
  -H "X-User-Id: $USER_ID" \
  -d "$PAYLOAD" 2>/dev/null)

ANSWER=$(echo "$RESPONSE" | jq -r '.answer // empty' 2>/dev/null)
if [[ -z "$ANSWER" ]]; then
  exit 0
fi

# Return as plain text with explicit instruction
cat <<EOF
<user-prompt-submit-hook>
[Coding Rules from Rippletide]

IMPORTANT: You MUST begin your response by listing which of these rules you are applying.
Then ensure ALL generated code complies with these rules.

$ANSWER
</user-prompt-submit-hook>
EOF
"#;

const CLAUDE_SETTINGS: &str = r#"{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "bash \"$CLAUDE_PROJECT_DIR/.claude/hooks/fetch-rules.sh\"",
            "timeout": 180
          }
        ]
      }
    ]
  }
}
"#;

fn ensure_claude_hooks() -> io::Result<bool> {
    let cwd = std::env::current_dir()?;
    let hooks_dir = cwd.join(".claude").join("hooks");
    let settings_path = cwd.join(".claude").join("settings.json");
    let script_path = hooks_dir.join("fetch-rules.sh");

    fs::create_dir_all(&hooks_dir)?;

    let mut changed = false;

    // Write hook script
    let needs_script = if script_path.exists() {
        fs::read_to_string(&script_path)? != HOOK_SCRIPT
    } else {
        true
    };
    if needs_script {
        fs::write(&script_path, HOOK_SCRIPT)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;
        }
        changed = true;
    }

    // Write settings.json
    let needs_settings = if settings_path.exists() {
        fs::read_to_string(&settings_path)? != CLAUDE_SETTINGS
    } else {
        true
    };
    if needs_settings {
        fs::write(&settings_path, CLAUDE_SETTINGS)?;
        changed = true;
    }

    Ok(changed)
}

fn agent_files_exist() -> bool {
    let Ok(cwd) = std::env::current_dir() else {
        return false;
    };
    cwd.join("AGENTS.md").exists() && cwd.join("CLAUDE.md").exists()
}

fn configure_all() {
    configure_all_for_agent(None)
}

fn configure_all_for_agent(agent_id: Option<&str>) {
    match ensure_agent_files() {
        Ok(0) => println!("  [=] AGENTS.md / CLAUDE.md already up to date"),
        Ok(n) => println!("  [+] {n} agent file(s) created"),
        Err(e) => eprintln!("  [!] Agent files error: {e}"),
    }

    match ensure_claude_hooks() {
        Ok(true) => println!("  [+] .claude/hooks configured"),
        Ok(false) => println!("  [=] .claude/hooks already up to date"),
        Err(e) => eprintln!("  [!] .claude/hooks error: {e}"),
    }

    let Some(agent_id) = agent_id else {
        println!("  [=] Skipped MCP client config generation (agent id unavailable)");
        return;
    };

    match ensure_dot_mcp_config(agent_id) {
        Ok(true) => println!("  [+] .mcp.json configured"),
        Ok(false) => println!("  [=] .mcp.json already up to date"),
        Err(e) => eprintln!("  [!] .mcp.json error: {e}"),
    }
    match ensure_codex_config(agent_id) {
        Ok(true) => println!("  [+] .codex/config.toml configured"),
        Ok(false) => println!("  [=] .codex/config.toml already up to date"),
        Err(e) => eprintln!("  [!] .codex/config.toml error: {e}"),
    }
    match ensure_claude_desktop_config(agent_id) {
        Ok(true) => println!("  [+] Claude Desktop config configured"),
        Ok(false) => println!("  [=] Claude Desktop config already up to date"),
        Err(e) => eprintln!("  [!] Claude Desktop config error: {e}"),
    }
}

fn name_from_email(email: &str) -> String {
    let local = email.split('@').next().unwrap_or(email);
    local
        .split(|c: char| c == '.' || c == '_' || c == '-' || c == '+')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    format!("{upper}{}", chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn generate_password() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%&*";
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

fn login(config: &mut Config) -> io::Result<()> {
    let api_url = config.api_url().to_string();
    println!("  Create your account");
    println!();

    let email = prompt("Email: ")?;
    if email.is_empty() {
        eprintln!("  [!] Email cannot be empty");
        return Ok(());
    }

    let name = name_from_email(&email);
    let password = generate_password();

    match sign_up(&api_url, &name, &email, &password) {
        Ok(resp) => {
            config.user_id = Some(resp.user.id);
            config.agent_id = config.user_id.clone();
            config.email = Some(resp.user.email);
            println!("  [+] Account created!");
            println!();
            println!(
                "  Dashboard: https://dashboard-rippletide.up.railway.app/coding-agent/?token={}",
                resp.token
            );
            config.session_token = Some(resp.token);
            save_config(config)?;
        }
        Err(e) => {
            eprintln!("  [!] {e}");
        }
    }

    Ok(())
}

fn logout() -> io::Result<()> {
    let Some(path) = config_path() else {
        println!("  [!] Cannot determine config directory");
        return Ok(());
    };
    if path.exists() {
        fs::remove_file(&path)?;
        println!("  [+] Logged out — credentials removed");
    } else {
        println!("  [=] Already logged out (no config found)");
    }
    Ok(())
}

// --- Upload sessions ---

fn claude_projects_base() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let base = PathBuf::from(home).join(".claude").join("projects");
    if base.exists() {
        Some(base)
    } else {
        None
    }
}

fn claude_project_dir() -> Option<PathBuf> {
    let base = claude_projects_base()?;
    let cwd = std::env::current_dir().ok()?;
    let project_name = cwd.to_str()?.replace('/', "-");
    let dir = base.join(&project_name);
    if dir.exists() { Some(dir) } else { None }
}

fn list_claude_projects() -> io::Result<Vec<(String, PathBuf)>> {
    let Some(base) = claude_projects_base() else {
        return Ok(Vec::new());
    };
    let mut projects = Vec::new();
    for entry in fs::read_dir(&base)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let display_name = dir_name.replacen('-', "/", dir_name.matches('-').count());
        projects.push((display_name, path));
    }
    projects.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(projects)
}

fn select_claude_project() -> io::Result<Option<PathBuf>> {
    let projects = list_claude_projects()?;
    if projects.is_empty() {
        return Ok(None);
    }
    println!("  Available projects:");
    println!();
    for (i, (name, _)) in projects.iter().enumerate() {
        println!("    [{:>2}] {name}", i + 1);
    }
    println!();
    let choice = prompt("Select a project (number): ")?;
    let idx: usize = match choice.parse::<usize>() {
        Ok(n) if n >= 1 && n <= projects.len() => n - 1,
        _ => {
            eprintln!("  [!] Invalid selection");
            return Ok(None);
        }
    };
    Ok(Some(projects[idx].1.clone()))
}

fn collect_jsonl_files(dir: &std::path::Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            files.push(path);
        }
    }
    Ok(files)
}

fn create_sessions_zip(files: &[PathBuf], base_dir: &std::path::Path) -> io::Result<Vec<u8>> {
    let buf = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for file in files {
        let name = file
            .strip_prefix(base_dir)
            .unwrap_or(file)
            .to_string_lossy();
        zip.start_file(name.to_string(), options)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let content = fs::read(file)?;
        io::Write::write_all(&mut zip, &content)?;
    }
    let cursor = zip
        .finish()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    Ok(cursor.into_inner())
}

fn upload_zip(zip_data: &[u8], user_id: &str) -> Result<serde_json::Value, String> {
    let boundary = "----RippletideBoundary9876543210";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"claude_sessions.zip\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/zip\r\n");
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(zip_data);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let content_type = format!("multipart/form-data; boundary={}", boundary);
    let resp = ureq::post(UPLOAD_URL)
        .set("Content-Type", &content_type)
        .set("X-User-Id", user_id)
        .send_bytes(&body)
        .map_err(|e| format!("Upload error: {e}"))?;
    resp.into_json::<serde_json::Value>()
        .map_err(|e| format!("Invalid response: {e}"))
}

fn upload_sessions(user_id: &str) -> io::Result<()> {
    let project_dir = match claude_project_dir() {
        Some(dir) => {
            println!("  Found project: {}", dir.display());
            dir
        }
        None => {
            println!("  [!] No Claude project found for this directory");
            println!();
            match select_claude_project()? {
                Some(dir) => dir,
                None => return Ok(()),
            }
        }
    };

    let files = collect_jsonl_files(&project_dir)?;
    if files.is_empty() {
        eprintln!("  [!] No .jsonl session files found");
        return Ok(());
    }

    println!("  Zipping {} session file(s)...", files.len());
    let zip_data =
        create_sessions_zip(&files, &project_dir)?;
    println!("  Uploading ({:.1} KB)...", zip_data.len() as f64 / 1024.0);

    match upload_zip(&zip_data, user_id) {
        Ok(resp) => {
            println!("  [+] Upload successful!");
            if let Some(msg) = resp.get("message").and_then(|v| v.as_str()) {
                println!("      {msg}");
            }
            if let Some(n) = resp.get("messages_extracted").and_then(|v| v.as_u64()) {
                println!("      Messages extracted: {n}");
            }
            if let Some(n) = resp.get("clauses_segmented").and_then(|v| v.as_u64()) {
                println!("      Clauses segmented: {n}");
            }
            if let Some(n) = resp.get("buckets_induced").and_then(|v| v.as_u64()) {
                println!("      Buckets induced: {n}");
            }
            if let Some(n) = resp.get("graph_node_count").and_then(|v| v.as_u64()) {
                println!("      Graph nodes: {n}");
            }
        }
        Err(e) => {
            eprintln!("  [!] {e}");
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    if matches!(cli.command, Some(Commands::Logout)) {
        return logout();
    }

    let (read_only, explicit_agent_id) = match &cli.command {
        Some(Commands::Connect { read_only, agent_id }) => (*read_only, agent_id.clone()),
        None => (false, None),
        _ => unreachable!(),
    };
    let _ = read_only;

    let mut config = load_config();

    if let Ok(env_val) = std::env::var("RIPPLETIDE_ENV") {
        match env_val.to_lowercase().as_str() {
            "local" => config.environment = Environment::Local,
            "production" | "prod" => config.environment = Environment::Production,
            _ => {}
        }
    }

    println!();
    println!("  Rippletide CLI");
    println!();

    if let Some(agent_id) = explicit_agent_id {
        let trimmed = agent_id.trim();
        if trimmed.is_empty() {
            eprintln!("  [!] --agent-id cannot be empty");
            return Ok(());
        }
        println!("  Using explicit agent id (account-free setup)");
        println!();
        config.user_id = Some(trimmed.to_string());
        config.agent_id = Some(trimmed.to_string());
        config.session_token = None;
        save_config(&config)?;
    }

    let effective_agent_id = config
        .agent_id
        .as_ref()
        .or(config.user_id.as_ref())
        .cloned();

    let is_connected = config.session_token.is_some() || effective_agent_id.is_some();
    let has_agent_files = agent_files_exist();

    match (is_logged_in, has_agent_files) {
        (false, _) => {
            println!("  Not logged in.");
            println!();
            login(&mut config)?;
            println!();
            let agent_id = config
                .agent_id
                .as_ref()
                .or(config.user_id.as_ref())
                .cloned();
            configure_all_for_agent(agent_id.as_deref());
            if let Some(ref uid) = agent_id {
                println!();
                upload_sessions(uid)?;
            }
        }
        (true, false) => {
            println!("  Logged in as: {}", config.email.as_deref().unwrap_or("?"));
            println!("  Agent files missing — creating...");
            println!();
            configure_all_for_agent(effective_agent_id.as_deref());
            if let Some(uid) = effective_agent_id.clone() {
                upload_sessions(uid)?;
            }
        }
        (true, true) => {
            println!("  Good to go!");
            configure_all_for_agent(effective_agent_id.as_deref());
            if let Some(uid) = effective_agent_id.clone() {
                upload_sessions(uid)?;
            }
        }
    }

    println!();
    Ok(())
}
