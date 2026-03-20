use std::path::Path;

use walkdir::WalkDir;

// rippletide-override: user approved
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TechStack {
    TypeScript,
    JavaScript,
    Python,
    Rust,
    Go,
    Ruby,
    Java,
    Kotlin,
    Swift,
    Cpp,
}

impl TechStack {
    pub fn label(&self) -> &str {
        match self {
            Self::TypeScript => "TypeScript",
            Self::JavaScript => "JavaScript",
            Self::Python => "Python",
            Self::Rust => "Rust",
            Self::Go => "Go",
            Self::Ruby => "Ruby",
            Self::Java => "Java",
            Self::Kotlin => "Kotlin",
            Self::Swift => "Swift",
            Self::Cpp => "C/C++",
        }
    }

}

pub struct RepoScanResult {
    pub source_file_count: usize,
    pub test_file_count: usize,
    pub has_claude_md: bool,
    pub mcp_tool_count: usize,
    pub tech_stacks: Vec<TechStack>,
}

const SKIP_DIRS: &[&str] = &["target", "node_modules", ".git", "dist", "build", "vendor"];

const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "rb", "c", "cpp", "h", "swift", "kt",
];

pub fn scan_repo(cwd: &Path) -> RepoScanResult {
    let mut source_file_count = 0;
    let mut test_file_count = 0;

    let walker = WalkDir::new(cwd).into_iter().filter_entry(|entry| {
        if entry.file_type().is_dir() {
            let name = entry.file_name().to_string_lossy();
            return !SKIP_DIRS.contains(&name.as_ref());
        }
        true
    });

    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e,
            None => continue,
        };

        if !SOURCE_EXTENSIONS.contains(&ext) {
            continue;
        }

        source_file_count += 1;

        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let path_str = path.to_string_lossy();

        let is_test = file_name.contains("test")
            || file_name.contains("_spec")
            || path_str.contains("/tests/")
            || path_str.contains("/__tests__/");

        if is_test {
            test_file_count += 1;
        }
    }

    let has_claude_md = cwd.join("CLAUDE.md").exists();
    let mcp_tool_count = count_mcp_tools(cwd);
    let tech_stacks = detect_tech_stacks(cwd);

    RepoScanResult {
        source_file_count,
        test_file_count,
        has_claude_md,
        mcp_tool_count,
        tech_stacks,
    }
}

fn detect_tech_stacks(cwd: &Path) -> Vec<TechStack> {
    // rippletide-override: user approved
    let indicators: &[(&str, TechStack)] = &[
        ("tsconfig.json", TechStack::TypeScript),
        ("package.json", TechStack::JavaScript),
        ("Cargo.toml", TechStack::Rust),
        ("go.mod", TechStack::Go),
        ("Gemfile", TechStack::Ruby),
        ("requirements.txt", TechStack::Python),
        ("pyproject.toml", TechStack::Python),
        ("setup.py", TechStack::Python),
        ("pom.xml", TechStack::Java),
        ("build.gradle", TechStack::Java),
        ("build.gradle.kts", TechStack::Kotlin),
        ("Package.swift", TechStack::Swift),
        ("CMakeLists.txt", TechStack::Cpp),
    ];

    let mut stacks = Vec::new();
    for (file, stack) in indicators {
        if cwd.join(file).exists() && !stacks.contains(stack) {
            stacks.push(stack.clone());
        }
    }

    // TypeScript supersedes JavaScript if both detected
    if stacks.contains(&TechStack::TypeScript) {
        stacks.retain(|s| *s != TechStack::JavaScript);
    }

    stacks
}

fn count_mcp_tools(cwd: &Path) -> usize {
    let mcp_path = cwd.join(".mcp.json");
    if !mcp_path.exists() {
        return 0;
    }
    let content = match std::fs::read_to_string(&mcp_path) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    // Count entries under "mcpServers" key
    json.get("mcpServers")
        .and_then(|v| v.as_object())
        .map(|obj| obj.len())
        .unwrap_or(0)
}
