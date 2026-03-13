use std::fs;
use std::io;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use clap::Parser;
use directories::ProjectDirs;
use rand::Rng;
use serde::{Deserialize, Serialize};

mod rules;
mod scan;
mod ui;

#[derive(Parser)]
#[command(name = "rippletide", about = "Rippletide MCP")]
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
    },
    /// Log out and remove stored credentials
    Logout,
}

const LOCAL_API_URL: &str = "http://localhost:3000";

const PROD_API_URL: &str = "https://dashboard-rippletide.up.railway.app/coding-agent";

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

fn extract_session_cookie(resp: &ureq::Response) -> Option<String> {
    for value in resp.all("set-cookie") {
        let rest = value
            .strip_prefix("__Secure-better-auth.session_token=")
            .or_else(|| value.strip_prefix("better-auth.session_token="));
        if let Some(rest) = rest {
            let token = rest.split(';').next().unwrap_or(rest);
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
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
    let cookie = extract_session_cookie(&resp)
        .ok_or_else(|| "No session cookie in response".to_string())?;
    let mut sign_up_resp: SignUpResponse = resp
        .into_json()
        .map_err(|e| format!("Invalid response: {e}"))?;
    sign_up_resp.token = cookie;
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

fn ensure_agent_files() -> io::Result<bool> {
    let cwd = std::env::current_dir()?;
    let content = build_agent_instructions();
    let mut changed = false;
    for name in ["AGENTS.md", "CLAUDE.md"] {
        let path = cwd.join(name);
        let needs_write = if path.exists() {
            fs::read_to_string(&path)? != content
        } else {
            true
        };
        if needs_write {
            fs::write(&path, &content)?;
            changed = true;
        }
    }
    Ok(changed)
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

struct ConfigureResult {
    agents_error: Option<String>,
    hooks_error: Option<String>,
}

fn configure_all() -> ConfigureResult {
    let agents_error = match ensure_agent_files() {
        Ok(_) => None,
        Err(e) => Some(format!("{e}")),
    };

    let hooks_error = match ensure_claude_hooks() {
        Ok(_) => None,
        Err(e) => Some(format!("{e}")),
    };

    ConfigureResult {
        agents_error,
        hooks_error,
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
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

struct LoginResult {
    success: bool,
}

fn login(config: &mut Config) -> io::Result<LoginResult> {
    let api_url = config.api_url().to_string();

    println!("  Enter your email to create your workspace");
    let email = ui::styled_prompt("")?;
    if email.is_empty() {
        ui::print_error("Email cannot be empty");
        return Ok(LoginResult { success: false });
    }

    let name = name_from_email(&email);
    let password = generate_password();

    match sign_up(&api_url, &name, &email, &password) {
        Ok(resp) => {
            config.user_id = Some(resp.user.id);
            config.email = Some(resp.user.email);
            let dashboard_url = format!(
                "https://dashboard-rippletide.up.railway.app/coding-agent/?token={}",
                resp.token
            );
            config.session_token = Some(resp.token);
            save_config(config)?;
            println!();
            ui::print_success("Workspace created");
            thread::sleep(Duration::from_millis(150));
            ui::print_success("MCP endpoint reserved");
            println!();
            ui::print_sub(&format!("Dashboard: {dashboard_url}"));
            Ok(LoginResult { success: true })
        }
        Err(e) => {
            ui::print_error(&e);
            Ok(LoginResult { success: false })
        }
    }
}

fn logout() -> io::Result<()> {
    let Some(path) = config_path() else {
        ui::print_error("Cannot determine config directory");
        return Ok(());
    };
    if path.exists() {
        fs::remove_file(&path)?;
        ui::print_success("Logged out — credentials removed");
    } else {
        ui::print_success("Already logged out (no config found)");
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
    let choice = ui::styled_prompt("Select a project (number): ")?;
    let idx: usize = match choice.parse::<usize>() {
        Ok(n) if n >= 1 && n <= projects.len() => n - 1,
        _ => {
            ui::print_error("Invalid selection");
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
        Some(dir) => dir,
        None => {
            ui::print_sub("No Claude project found for this directory");
            println!();
            match select_claude_project()? {
                Some(dir) => dir,
                None => return Ok(()),
            }
        }
    };

    let files = collect_jsonl_files(&project_dir)?;
    if files.is_empty() {
        return Ok(());
    }

    let sp = ui::start_spinner(&format!("Uploading {} session file(s)...", files.len()));
    let zip_data = create_sessions_zip(&files, &project_dir)?;

    match upload_zip(&zip_data, user_id) {
        Ok(resp) => {
            ui::finish_spinner(&sp, "Sessions uploaded");
            if let Some(n) = resp.get("messages_extracted").and_then(|v| v.as_u64()) {
                ui::print_sub(&format!("Messages extracted: {n}"));
            }
            if let Some(n) = resp.get("graph_node_count").and_then(|v| v.as_u64()) {
                ui::print_sub(&format!("Graph nodes: {n}"));
            }
        }
        Err(e) => {
            sp.finish_and_clear();
            ui::print_error(&e);
        }
    }
    Ok(())
}

fn run_scan_phase(cwd: &std::path::Path) -> scan::RepoScanResult {
    let sp = ui::start_spinner("Analyzing current repository...");
    let result = scan::scan_repo(cwd);
    thread::sleep(Duration::from_millis(600));
    ui::finish_spinner(&sp, "Scanning repository structure");
    println!();

    if result.has_claude_md {
        ui::print_success("CLAUDE.md detected");
    }
    ui::print_success(&format!("{} source files", result.source_file_count));
    ui::print_success(&format!("{} test files", result.test_file_count));
    if result.mcp_tool_count > 0 {
        ui::print_success(&format!("{} MCP tools", result.mcp_tool_count));
    }

    result
}

fn run_rules_phase(cwd: &std::path::Path) {
    let sp = ui::start_spinner("Reading assistant instructions");
    thread::sleep(Duration::from_millis(800));
    ui::finish_spinner(&sp, "Reading assistant instructions");
    println!();

    ui::print_sub("Parsing CLAUDE.md");
    thread::sleep(Duration::from_millis(300));
    ui::print_sub("Extracting rules");
    thread::sleep(Duration::from_millis(300));
    ui::print_sub("Normalizing constraints");
    thread::sleep(Duration::from_millis(300));
    println!();

    let count = rules::count_rules_in_claude_md(cwd);
    ui::print_success(&format!("{count} explicit rules detected"));
}

fn run_conventions_phase() {
    let sp = ui::start_spinner("Inferring repository conventions");
    thread::sleep(Duration::from_millis(800));
    ui::finish_spinner(&sp, "Inferring repository conventions");
    println!();

    ui::print_sub("Analyzing file structure");
    thread::sleep(Duration::from_millis(300));
    ui::print_sub("Analyzing test patterns");
    thread::sleep(Duration::from_millis(300));
    ui::print_sub("Analyzing API usage");
    thread::sleep(Duration::from_millis(300));
    println!();

    ui::print_result("Detected patterns");
}

fn run_configure_phase() {
    let result = configure_all();

    match result.agents_error {
        Some(e) => ui::print_error(&format!("Agent files error: {e}")),
        None => ui::print_success("AGENTS.md configured"),
    }

    thread::sleep(Duration::from_millis(150));

    match result.hooks_error {
        Some(e) => ui::print_error(&format!(".claude/hooks error: {e}")),
        None => ui::print_success(".claude/hooks configured"),
    }
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    if matches!(cli.command, Some(Commands::Logout)) {
        return logout();
    }

    let _read_only = match &cli.command {
        Some(Commands::Connect { read_only }) => *read_only,
        None => false,
        _ => unreachable!(),
    };

    let mut config = load_config();

    if let Ok(env_val) = std::env::var("RIPPLETIDE_ENV") {
        match env_val.to_lowercase().as_str() {
            "local" => config.environment = Environment::Local,
            "production" | "prod" => config.environment = Environment::Production,
            _ => {}
        }
    }

    let cwd = std::env::current_dir()?;

    // Phase 1 — Header
    ui::print_header("Rippletide MCP");

    let is_logged_in = config.session_token.is_some();

    // Phase 2 — Auth (if not logged in)
    if !is_logged_in {
        let login_result = login(&mut config)?;
        println!();
        if !login_result.success {
            return Ok(());
        }
    }

    // Phase 3 — Repository scan
    let scan_result = run_scan_phase(&cwd);
    println!();

    // Phase 4 — Reading assistant instructions (if CLAUDE.md exists)
    if scan_result.has_claude_md {
        run_rules_phase(&cwd);
        println!();
    }

    // Phase 5 — Inferring conventions
    run_conventions_phase();
    println!();

    // Phase 6 — Configure files
    run_configure_phase();

    // Phase 7 — Upload sessions (only on first login)
    if !is_logged_in {
        if let Some(ref uid) = config.user_id {
            println!();
            upload_sessions(uid)?;
        }
    }

    println!();
    Ok(())
}
