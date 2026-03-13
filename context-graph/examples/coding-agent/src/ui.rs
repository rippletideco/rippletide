use std::io;
use std::io::Write;
use std::thread;
use std::time::Duration;

use colored::Colorize;
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
