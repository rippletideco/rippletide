// rippletide-override: user approved
use std::path::Path;

use serde::Deserialize;

use crate::scan::TechStack;

const CORPUS_JSON: &str = include_str!("corpus_rules.json");

const MIN_FREQUENCY_THRESHOLD: usize = 5;
const TOP_MISSING_COUNT: usize = 3;

#[derive(Debug, Clone, Deserialize)]
pub struct CorpusRule {
    pub frequency: usize,
    pub rule: String,
    #[serde(rename = "type")]
    pub rule_type: String,
    pub category: String,
    // rippletide-override: user approved
    #[serde(default)]
    #[allow(dead_code)]
    pub variants: Vec<String>,
}

#[derive(Debug, Clone)]
// rippletide-override: user approved
pub struct MissingRule {
    // rippletide-override: user approved
    pub rule: String,
    #[allow(dead_code)]
    pub frequency: usize,
    #[allow(dead_code)]
    pub category: String,
    #[allow(dead_code)]
    pub rule_type: String,
}

fn load_corpus() -> Vec<CorpusRule> {
    serde_json::from_str(CORPUS_JSON).unwrap_or_default()
}

/// Returns true if a rule is relevant to the given tech stacks.
/// Rules that don't mention any specific tech are considered universal.
fn is_relevant_to_stacks(rule: &CorpusRule, stacks: &[TechStack]) -> bool {
    let rule_lower = rule.rule.to_lowercase();

    // All known tech keywords mapped to their stack
    let all_tech_keywords: &[(&[&str], TechStack)] = &[
        (&["typescript", "tsx", "jsx", "eslint", "prettier", "react", "next.js", "angular", "vue"], TechStack::TypeScript),
        (&["python", "pytest", "mypy", "pylint", "django", "flask"], TechStack::Python),
        (&["rust", "cargo", "clippy"], TechStack::Rust),
        (&["golang", "goroutine"], TechStack::Go),
        (&["ruby", "rails", "rspec"], TechStack::Ruby),
        (&["java ", "maven", "spring", "junit"], TechStack::Java),
        (&["kotlin", "gradle"], TechStack::Kotlin),
        (&["swift", "xcode", "swiftui"], TechStack::Swift),
        (&["cmake", "clang"], TechStack::Cpp),
    ];

    // Check if the rule mentions any tech-specific keyword
    let mut mentions_any_tech = false;
    let mut matches_user_stack = false;

    for (keywords, stack) in all_tech_keywords {
        for kw in *keywords {
            if rule_lower.contains(kw) {
                mentions_any_tech = true;
                if stacks.contains(stack) {
                    matches_user_stack = true;
                }
            }
        }
    }

    // Universal rule (no tech keywords) or matches user's stack
    !mentions_any_tech || matches_user_stack
}

/// Filters the corpus rules to those relevant to the detected tech stacks
/// and above the minimum frequency threshold.
// rippletide-override: user approved
fn filter_candidate_rules<'a>(corpus: &'a [CorpusRule], stacks: &[TechStack]) -> Vec<&'a CorpusRule> {
    let mut candidates: Vec<&CorpusRule> = corpus
        .iter()
        .filter(|r| r.frequency >= MIN_FREQUENCY_THRESHOLD)
        .filter(|r| is_relevant_to_stacks(r, stacks))
        .collect();
    candidates.sort_by(|a, b| b.frequency.cmp(&a.frequency));
    candidates
}

/// Uses the Claude CLI to identify which candidate rules are NOT covered
/// by the user's CLAUDE.md content.
pub fn find_missing_rules(
    cwd: &Path,
    candidates: &[&CorpusRule],
    claude_md_content: &str,
    claude_fn: &dyn Fn(&Path, &str) -> Result<String, String>,
) -> Vec<MissingRule> {
    if candidates.is_empty() || claude_md_content.trim().is_empty() {
        // If no CLAUDE.md, all candidates are missing
        return candidates
            .iter()
            .take(TOP_MISSING_COUNT)
            .map(|r| MissingRule {
                rule: r.rule.clone(),
                frequency: r.frequency,
                category: r.category.clone(),
                rule_type: r.rule_type.clone(),
            })
            .collect();
    }

    let rules_list: String = candidates
        .iter()
        .enumerate()
        .map(|(i, r)| format!("{}. [freq={}] {}", i + 1, r.frequency, r.rule))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "You are comparing a user's CLAUDE.md against a corpus of common coding rules.\n\n\
         User's CLAUDE.md:\n---\n{claude_md_content}\n---\n\n\
         Candidate rules from the corpus (ranked by frequency):\n{rules_list}\n\n\
         Identify which candidate rules are NOT already covered (explicitly or implicitly) \
         by the user's CLAUDE.md. A rule is \"covered\" if the CLAUDE.md already contains \
         guidance that addresses the same concern, even if worded differently.\n\n\
         Output ONLY valid JSON: an array of the NUMBERS (1-indexed) of the missing rules.\n\
         Example: [1, 5, 12]\n\
         If all rules are covered, output: []"
    );

    let raw = match claude_fn(cwd, &prompt) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let missing_indices = parse_missing_indices(&raw);

    missing_indices
        .into_iter()
        .filter_map(|idx| {
            let i = idx.checked_sub(1)?;
            candidates.get(i).map(|r| MissingRule {
                rule: r.rule.clone(),
                frequency: r.frequency,
                category: r.category.clone(),
                rule_type: r.rule_type.clone(),
            })
        })
        .take(TOP_MISSING_COUNT)
        .collect()
}

fn parse_missing_indices(raw: &str) -> Vec<usize> {
    let trimmed = raw.trim();
    // Find the JSON array in the response
    let start = match trimmed.find('[') {
        Some(s) => s,
        None => return Vec::new(),
    };
    let end = match trimmed.rfind(']') {
        Some(e) => e,
        None => return Vec::new(),
    };
    if start >= end {
        return Vec::new();
    }
    let json_str = &trimmed[start..=end];
    serde_json::from_str::<Vec<usize>>(json_str).unwrap_or_default()
}

/// Run the full missing rules benchmark.
/// Returns the top missing rules for the user's tech stack.
pub fn run_benchmark(
    cwd: &Path,
    stacks: &[TechStack],
    claude_fn: &dyn Fn(&Path, &str) -> Result<String, String>,
) -> Vec<MissingRule> {
    let corpus = load_corpus();
    let candidates = filter_candidate_rules(&corpus, stacks);

    if candidates.is_empty() {
        return Vec::new();
    }

    let claude_md_path = cwd.join("CLAUDE.md");
    let claude_md_content = std::fs::read_to_string(&claude_md_path).unwrap_or_default();

    find_missing_rules(cwd, &candidates, &claude_md_content, claude_fn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_corpus_successfully() {
        let corpus = load_corpus();
        assert!(!corpus.is_empty());
        assert!(corpus[0].frequency > 0);
    }

    #[test]
    fn filters_by_tech_stack() {
        let corpus = load_corpus();
        let ts_stacks = vec![TechStack::TypeScript];
        let candidates = filter_candidate_rules(&corpus, &ts_stacks);
        // Should include universal rules and TS-specific rules
        assert!(!candidates.is_empty());
        // Should not include rules specific to other stacks
        for c in &candidates {
            let lower = c.rule.to_lowercase();
            assert!(
                !lower.contains("python") && !lower.contains("pytest"),
                "Python-specific rule should be excluded for TS stack: {}",
                c.rule
            );
        }
    }

    #[test]
    fn filters_by_rust_stack() {
        let corpus = load_corpus();
        let stacks = vec![TechStack::Rust];
        let candidates = filter_candidate_rules(&corpus, &stacks);
        assert!(!candidates.is_empty());
        for c in &candidates {
            let lower = c.rule.to_lowercase();
            assert!(
                !lower.contains("typescript"),
                "TS-specific rule should be excluded for Rust stack: {}",
                c.rule
            );
        }
    }

    #[test]
    fn candidates_sorted_by_frequency_desc() {
        let corpus = load_corpus();
        let candidates = filter_candidate_rules(&corpus, &[]);
        for window in candidates.windows(2) {
            assert!(window[0].frequency >= window[1].frequency);
        }
    }

    #[test]
    fn parses_missing_indices() {
        assert_eq!(parse_missing_indices("[1, 3, 5]"), vec![1, 3, 5]);
        assert_eq!(parse_missing_indices("Some text [2, 4] more text"), vec![2, 4]);
        assert_eq!(parse_missing_indices("[]"), Vec::<usize>::new());
        assert_eq!(parse_missing_indices("no array here"), Vec::<usize>::new());
    }

    #[test]
    fn returns_top_3_when_no_claude_md() {
        let corpus = load_corpus();
        let candidates = filter_candidate_rules(&corpus, &[]);
        let missing = find_missing_rules(
            Path::new("/tmp"),
            &candidates,
            "",
            &|_, _| Ok("[]".to_string()),
        );
        assert_eq!(missing.len(), 3);
        // Should be the top 3 by frequency
        assert!(missing[0].frequency >= missing[1].frequency);
        assert!(missing[1].frequency >= missing[2].frequency);
    }
}
