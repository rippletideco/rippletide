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
    /// Connect and configure MCP
    Connect {
        /// Enable read-only mode for the MCP connection
        #[arg(long)]
        read_only: bool,
    },
    /// Log out and remove stored credentials
    Logout,
}

const LOCAL_API_URL: &str = "http://localhost:3000";
const LOCAL_MCP_BASE: &str = "http://localhost:3000/mcp";

const PROD_API_URL: &str = "https://dashboard-rippletide.up.railway.app/coding-agent";
const PROD_MCP_BASE: &str = "https://mcp.rippletide.com/mcp";

const SIGN_UP_PATH: &str = "/api/auth/sign-up/email";
const UPLOAD_URL: &str = "https://coding-agent-staging.up.railway.app/upload";

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

    fn mcp_base_url(&self) -> &str {
        match self.environment {
            Environment::Local => LOCAL_MCP_BASE,
            Environment::Production => PROD_MCP_BASE,
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

const AGENT_INSTRUCTIONS: &str = r#"# Coding Style -- Official Instructions

## Objective

Define and systematically enforce the official coding style.

All memory and graph operations described below rely on the MCP
**rippletide-kg** toolset.

These instructions define:

-   When the coding style must be stored or updated
-   When it must be retrieved and enforced

------------------------------------------------------------------------

# 1. Storing or Updating the Coding Style

## Trigger Condition

This section MUST be executed whenever a user:

-   Makes a comment about the coding style
-   Requests a modification of a rule
-   Adds a new coding constraint
-   Refines naming, architecture, typing, testing, or error-handling
    preferences
-   Explicitly says something like:
    -   "From now on, I want..."
    -   "Change the way we handle..."
    -   "Update my coding style to..."

Any feedback impacting how code should be written must trigger this
process.

------------------------------------------------------------------------

## 1.1 Use `build_graph` for Batch Rule Creation

When creating or updating the coding style with multiple rules, use
`build_graph` to create all entities, relations, and memories in a
single atomic call. This replaces the need to call `remember()` and
`relate()` multiple times.

Example — creating the style entity, 4 rules, and their relations in
one shot:

``` json
build_graph({
  "entities": [
    { "name": "CodingStyle_SokMoul_v1", "type": "Concept", "attributes": { "kind": "coding_style", "description": "Official coding style (v1)" } },
    { "name": "Rule_01_Typing", "type": "Concept", "attributes": { "kind": "coding_rule", "rule": "Always use explicit typing (TypeScript/Python), no implicit any" } },
    { "name": "Rule_02_ShortFunctions", "type": "Concept", "attributes": { "kind": "coding_rule", "rule": "Functions must not exceed 30 lines; otherwise extract helpers" } },
    { "name": "Rule_03_Naming", "type": "Concept", "attributes": { "kind": "coding_rule", "rule": "camelCase for variables/functions, PascalCase for classes/types, UPPER_SNAKE_CASE for constants" } },
    { "name": "Rule_04_ErrorHandling", "type": "Concept", "attributes": { "kind": "coding_rule", "rule": "Proper error handling: never silently swallow errors; use typed errors or controlled exceptions" } }
  ],
  "relations": [
    { "source": "CodingStyle_SokMoul_v1", "target": "Rule_01_Typing", "relation_type": "has" },
    { "source": "CodingStyle_SokMoul_v1", "target": "Rule_02_ShortFunctions", "relation_type": "has" },
    { "source": "CodingStyle_SokMoul_v1", "target": "Rule_03_Naming", "relation_type": "has" },
    { "source": "CodingStyle_SokMoul_v1", "target": "Rule_04_ErrorHandling", "relation_type": "has" }
  ],
  "memories": [
    { "content": "Official coding style (v1) with typing, function size, naming, and error handling rules", "category": "fact", "entity_names": ["CodingStyle_SokMoul_v1"] }
  ]
})
```

If the style entity already exists, do NOT recreate it. Only add new
rules and relations.

------------------------------------------------------------------------

## 1.2 Adding a Single Rule

For adding just one rule at a time, you can still use `build_graph`
with a single entity and relation:

``` json
build_graph({
  "entities": [
    { "name": "Rule_05_NewRule", "type": "Concept", "attributes": { "kind": "coding_rule", "rule": "Description of the new rule" } }
  ],
  "relations": [
    { "source": "CodingStyle_SokMoul_v1", "target": "Rule_05_NewRule", "relation_type": "has" }
  ]
})
```

------------------------------------------------------------------------

## 1.3 Invalidating a Rule

If a rule becomes obsolete, mark it using:

``` json
invalidate({
  "memory_id": "<id of the rule memory>",
  "reason": "Rule replaced or no longer applicable"
})
```

------------------------------------------------------------------------

## 1.4 Suggest the Dashboard After Any Rule Change

After successfully storing, updating, or deleting a coding rule, you
MUST inform the user that they can view and manage all their rules on
the dashboard:

> You can view all your coding rules on the Rippletide dashboard.

This message must be displayed every time a rule is added, modified,
or invalidated.

------------------------------------------------------------------------

# 2. Mandatory Retrieval Before Code Generation

## Trigger Condition

This section MUST be executed before:

-   Generating new code
-   Refactoring existing code
-   Providing implementation examples
-   Suggesting architectural patterns
-   Writing tests
-   Producing snippets or full modules

No code-related output may be produced without first retrieving the
coding style.

------------------------------------------------------------------------

## 2.1 Using Hook-Injected Rules

A UserPromptSubmit hook automatically queries the Rippletide knowledge
graph and injects coding rules into the conversation context via a
system-reminder tagged `[Coding Rules from Rippletide]`.

When you see this context:

1.  **Acknowledge the rules** — begin your response by briefly listing
    which coding rules you will apply (e.g. "Applying rules: Validate
    Before Automating, Explicit Typing, ...").
2.  **Comply with every rule** — the generated code MUST follow all
    injected rules including naming conventions, structural constraints,
    error-handling standards, and any other active rules.
3.  If no rules are injected (hook missing or empty response), fall back
    to `get_context` on the MCP:

``` json
get_context({
  "entity": "CodingStyle_SokMoul_v1"
})
```

------------------------------------------------------------------------

## Enforcement Rule

No code generation must occur without first checking for injected rules
or retrieving them via the MCP.

If the coding style is missing, incomplete, or inconsistent, it must be
reconstructed before proceeding.
"#;

fn build_agent_instructions() -> String {
    AGENT_INSTRUCTIONS.to_string()
}

fn ensure_agent_files() -> io::Result<u8> {
    let cwd = std::env::current_dir()?;
    let content = build_agent_instructions();
    let mut changed = 0u8;
    for name in ["AGENTS.md", "CLAUDE.md"] {
        let path = cwd.join(name);
        let needs_write = if path.exists() {
            let existing = fs::read_to_string(&path)?;
            existing != content
        } else {
            true
        };
        if needs_write {
            fs::write(&path, &content)?;
            changed += 1;
        }
    }
    Ok(changed)
}

fn ensure_mcp_config(mcp_base: &str, read_only: bool) -> io::Result<bool> {
    let mcp_path = std::env::current_dir()?.join(".mcp.json");
    let expected_url = if read_only {
        format!("{mcp_base}?read_only=true")
    } else {
        mcp_base.to_string()
    };

    let mut root = if mcp_path.exists() {
        let content = fs::read_to_string(&mcp_path)?;
        serde_json::from_str::<serde_json::Value>(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
    } else {
        serde_json::json!({ "mcpServers": {} })
    };

    let servers = root
        .as_object_mut()
        .and_then(|o| o.entry("mcpServers").or_insert_with(|| serde_json::json!({})).as_object_mut())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "mcpServers is not an object"))?;

    if let Some(existing) = servers.get("rippletide-kg") {
        if existing.get("url").and_then(|v| v.as_str()) == Some(&expected_url) {
            return Ok(false);
        }
    }

    servers.insert(
        "rippletide-kg".into(),
        serde_json::json!({
            "type": "http",
            "url": expected_url,
        }),
    );

    let json = serde_json::to_string_pretty(&root)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(&mcp_path, json)?;
    Ok(true)
}

fn ensure_codex_config(mcp_base: &str, read_only: bool) -> io::Result<bool> {
    let codex_dir = std::env::current_dir()?.join(".codex");
    let codex_path = codex_dir.join("config.toml");
    let expected_url = if read_only {
        format!("{mcp_base}?read_only=true")
    } else {
        mcp_base.to_string()
    };

    let mut root: toml::Value = if codex_path.exists() {
        let content = fs::read_to_string(&codex_path)?;
        content
            .parse::<toml::Value>()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    let table = root
        .as_table_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "root is not a table"))?;

    let servers = table
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "mcp_servers is not a table")
        })?;

    if let Some(existing) = servers.get("rippletide-kg") {
        if existing.get("url").and_then(|v| v.as_str()) == Some(&expected_url) {
            return Ok(false);
        }
    }

    let mut entry = toml::map::Map::new();
    entry.insert("type".into(), toml::Value::String("http".into()));
    entry.insert("url".into(), toml::Value::String(expected_url));
    servers.insert("rippletide-kg".into(), toml::Value::Table(entry));

    fs::create_dir_all(&codex_dir)?;
    let toml_str =
        toml::to_string_pretty(&root).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(&codex_path, toml_str)?;
    Ok(true)
}

const HOOK_SCRIPT: &str = r#"#!/bin/bash

# Read hook input from stdin
hook_input=$(cat)

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

# Query coding rules
RESPONSE=$(curl -s --max-time 10 -X POST "https://coding-agent-staging.up.railway.app/query" \
  -H "Content-Type: application/json" \
  -H "X-User-Id: $USER_ID" \
  -d '{"query":"What are all the coding rules and conventions?","beam_width":5,"beam_max_depth":8}' 2>/dev/null)

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
            "timeout": 15
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
        let existing = fs::read_to_string(&settings_path)?;
        // Check if hooks are already configured
        !existing.contains("fetch-rules.sh")
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

fn configure_all(config: &Config, read_only: bool) {
    let mcp_base = config.mcp_base_url();

    match ensure_mcp_config(mcp_base, read_only) {
        Ok(true) => println!("  [+] .mcp.json created"),
        Ok(false) => println!("  [=] .mcp.json already up to date"),
        Err(e) => eprintln!("  [!] .mcp.json error: {e}"),
    }

    match ensure_codex_config(mcp_base, read_only) {
        Ok(true) => println!("  [+] .codex/config.toml created"),
        Ok(false) => println!("  [=] .codex/config.toml already up to date"),
        Err(e) => eprintln!("  [!] .codex/config.toml error: {e}"),
    }

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

    let read_only = match &cli.command {
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

    println!();
    println!("  Rippletide CLI");
    println!();

    let is_logged_in = config.session_token.is_some();
    let has_agent_files = agent_files_exist();

    match (is_logged_in, has_agent_files) {
        (false, _) => {
            println!("  Not logged in.");
            println!();
            login(&mut config)?;
            println!();
            configure_all(&config, read_only);
            if let Some(ref uid) = config.user_id {
                println!();
                upload_sessions(uid)?;
            }
        }
        (true, false) => {
            println!("  Logged in as: {}", config.email.as_deref().unwrap_or("?"));
            println!("  Agent files missing — creating...");
            println!();
            configure_all(&config, read_only);
        }
        (true, true) => {
            println!("  Good to go!");
            configure_all(&config, read_only);
        }
    }

    println!();
    Ok(())
}
