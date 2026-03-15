use anyhow::Result;
use console::Style;
use std::collections::VecDeque;
use std::io::{BufRead, Seek, SeekFrom};
use std::path::PathBuf;
use tokio::sync::broadcast;

use crate::runner::format_log_line;

/// Show the last N lines from a log file with colored prefix
pub fn show_last_lines(name: &str, log_file: &PathBuf, color: &Style, lines: usize) -> Result<()> {
    use console::style;

    if !log_file.exists() {
        let prefix = color.apply_to(format!("[{}]", name));
        println!("{} {}", prefix, style("(no logs yet)").dim());
        return Ok(());
    }

    let file = std::fs::File::open(log_file)?;
    let reader = std::io::BufReader::new(file);

    let mut last_lines: VecDeque<String> = VecDeque::with_capacity(lines);
    for line in reader.lines().map_while(Result::ok) {
        if last_lines.len() >= lines {
            last_lines.pop_front();
        }
        last_lines.push_back(line);
    }

    for line in last_lines {
        println!("{}", format_log_line(name, &line, color));
    }

    Ok(())
}

/// Tail a log file, printing new lines with colored prefix.
/// Blocks until shutdown signal or error.
pub async fn tail_log_file(
    name: &str,
    log_file: &PathBuf,
    color: &Style,
    shutdown_rx: &mut broadcast::Receiver<()>,
) -> Result<()> {
    // Wait for file to exist
    while !log_file.exists() {
        tokio::select! {
            _ = shutdown_rx.recv() => return Ok(()),
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
        }
    }

    // Open file and seek to end
    let file = tokio::fs::File::open(log_file).await?;
    let metadata = file.metadata().await?;
    let mut pos = metadata.len();

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                let file = tokio::fs::File::open(log_file).await?;
                let metadata = file.metadata().await?;
                let new_len = metadata.len();

                if new_len > pos {
                    let mut file = std::fs::File::open(log_file)?;
                    file.seek(SeekFrom::Start(pos))?;

                    let reader = std::io::BufReader::new(file);
                    for line in reader.lines().map_while(Result::ok) {
                        println!("{}", format_log_line(name, &line, color));
                    }
                    pos = new_len;
                } else if new_len < pos {
                    pos = 0;
                }
            }
        }
    }

    Ok(())
}
