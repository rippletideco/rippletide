use std::io;
use std::io::Write;
use std::thread;
use std::time::Duration;

use colored::Colorize;
use console::{Key, Term};
use indicatif::{ProgressBar, ProgressStyle};

const LINE_DELAY: Duration = Duration::from_millis(100);

fn pause() {
    thread::sleep(LINE_DELAY);
}

pub fn print_header(text: &str) {
    println!();
    println!("  {}", text.green().bold());
    println!();
    pause();
}

pub fn print_success(text: &str) {
    pause();
    println!("  {} {}", "✓".green(), text.green());
}

pub fn print_progress(text: &str) {
    pause();
    println!("  {} {}", "✓".yellow(), text.yellow());
}

pub fn print_sub(text: &str) {
    pause();
    println!("    {}", text.dimmed());
}

pub fn print_result(text: &str) {
    pause();
    println!("  {}", text.yellow());
}

pub fn print_info(text: &str) {
    pause();
    println!("  {} {}", "→".cyan(), text.cyan());
}

pub fn print_error(text: &str) {
    eprintln!("  {}", text.red());
}

pub fn styled_prompt(label: &str) -> io::Result<String> {
    print!("  {} ", ">".white());
    print!("{}", label.white());
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn truncate_single_line(text: &str, max_chars: usize) -> String {
    let clean = text.replace(['\r', '\n'], " ");
    let mut chars = clean.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", truncated.trim_end())
    } else {
        truncated
    }
}

fn wrap_for_preview(text: &str, width: usize, max_lines: usize) -> Vec<String> {
    let clean = text.replace(['\r', '\n'], " ");
    let words: Vec<&str> = clean.split_whitespace().collect();
    if words.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in words {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current, word)
        };
        if candidate.chars().count() > width && !current.is_empty() {
            lines.push(current);
            current = word.to_string();
            if lines.len() == max_lines {
                break;
            }
        } else {
            current = candidate;
        }
    }
    if lines.len() < max_lines && !current.is_empty() {
        lines.push(current);
    }
    if lines.len() == max_lines {
        let consumed = lines.join(" ");
        if consumed.chars().count() < clean.chars().count() {
            if let Some(last) = lines.last_mut() {
                if !last.ends_with('…') {
                    last.push('…');
                }
            }
        }
    }
    lines
}

pub fn prompt_multi_select(
    title: &str,
    subtitle: &[&str],
    items: &[String],
) -> io::Result<Vec<usize>> {
    prompt_multi_select_with_term(
        Term::stdout(),
        title,
        subtitle,
        items,
        "Selected rule preview",
        &[],
    )
}

pub fn prompt_multi_select_to_stderr(
    title: &str,
    subtitle: &[&str],
    items: &[String],
    preview_label: &str,
    default_selected: &[usize],
) -> io::Result<Vec<usize>> {
    prompt_multi_select_with_term(
        Term::stderr(),
        title,
        subtitle,
        items,
        preview_label,
        default_selected,
    )
}

fn prompt_multi_select_with_term(
    term: Term,
    title: &str,
    subtitle: &[&str],
    items: &[String],
    preview_label: &str,
    default_selected: &[usize],
) -> io::Result<Vec<usize>> {
    let mut cursor: usize = 0;
    let mut selected = vec![false; items.len()];
    for index in default_selected {
        if *index < selected.len() {
            selected[*index] = true;
        }
    }

    term.write_line("")?;
    term.write_line(&format!("  {}", title.green().bold()))?;
    term.write_line("")?;
    for line in subtitle {
        term.write_line(&format!("    {}", line.dimmed()))?;
    }
    term.write_line("")?;

    let help_line = "  ↑/↓ move • space toggle • enter confirm";
    let item_width = 92usize;
    let preview_width = 96usize;
    let preview_max_lines = 4usize;
    let total_lines = items.len() + preview_max_lines + 4;

    loop {
        for (idx, item) in items.iter().enumerate() {
            let is_cursor = idx == cursor;
            let checkbox = if selected[idx] { "[x]" } else { "[ ]" };
            let prefix = if is_cursor {
                ">".cyan().to_string()
            } else {
                " ".to_string()
            };
            let line = format!(
                "  {} {} {}",
                prefix,
                checkbox,
                truncate_single_line(item, item_width)
            );
            if is_cursor {
                term.write_line(&format!("{}", line.black().on_white()))?;
            } else {
                term.write_line(&line)?;
            }
        }
        term.write_line("")?;
        term.write_line(&format!("  {}", preview_label.dimmed()))?;
        let preview_lines = if items.is_empty() {
            vec![String::new()]
        } else {
            wrap_for_preview(&items[cursor], preview_width, preview_max_lines)
        };
        for i in 0..preview_max_lines {
            let line = preview_lines.get(i).cloned().unwrap_or_default();
            term.write_line(&format!("    {}", line.dimmed()))?;
        }
        term.write_line("")?;
        term.write_line(help_line)?;

        match term.read_key()? {
            Key::ArrowUp => {
                cursor = cursor.saturating_sub(1);
            }
            Key::ArrowDown => {
                if cursor + 1 < items.len() {
                    cursor += 1;
                }
            }
            Key::Char(' ') => {
                if !items.is_empty() {
                    selected[cursor] = !selected[cursor];
                }
            }
            Key::Enter => {
                term.clear_last_lines(total_lines)?;
                break;
            }
            Key::Char('a') | Key::Char('A') => {
                let all_selected = selected.iter().all(|v| *v);
                for value in &mut selected {
                    *value = !all_selected;
                }
            }
            _ => {}
        }

        term.clear_last_lines(total_lines)?;
    }

    Ok(selected
        .iter()
        .enumerate()
        .filter_map(|(idx, keep)| if *keep { Some(idx) } else { None })
        .collect())
}

pub fn start_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("  {spinner:.yellow} {msg:.yellow}")
            .expect("invalid spinner template")
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

pub fn finish_spinner(pb: &ProgressBar, msg: &str) {
    pb.finish_and_clear();
    print_progress(msg);
}
