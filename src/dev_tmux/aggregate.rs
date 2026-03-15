use anyhow::Result;
use std::collections::HashSet;
use tokio::sync::broadcast;

use crate::config;
use crate::log_tailer;
use crate::runner::get_color_for_index;

/// Run the aggregate log view — tails all log files in the project's log directory.
/// Periodically checks for new log files (handles race with pipe-pane setup).
pub async fn run(project: &str) -> Result<()> {
    let logs_dir = config::get_project_logs_dir(project);

    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        let _ = shutdown_tx_clone.send(());
    });

    let mut known_files: HashSet<std::path::PathBuf> = HashSet::new();
    let mut color_index: usize = 0;

    // Main loop: discover new log files and start tailers
    let mut handles = Vec::new();
    loop {
        // Discover log files
        if logs_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&logs_dir) {
                let mut new_files: Vec<_> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.extension().is_some_and(|e| e == "log") && !known_files.contains(p)
                    })
                    .collect();
                new_files.sort();

                for log_file in new_files {
                    let service_name = log_file
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let color = get_color_for_index(color_index);
                    color_index += 1;
                    known_files.insert(log_file.clone());
                    let mut shutdown_rx = shutdown_tx.subscribe();

                    let handle = tokio::spawn(async move {
                        if let Err(e) =
                            log_tailer::tail_log_file(&service_name, &log_file, &color, &mut shutdown_rx).await
                        {
                            eprintln!("Error tailing {}: {}", service_name, e);
                        }
                    });
                    handles.push(handle);
                }
            }
        }

        // Check if shutdown was requested
        let mut check_rx = shutdown_tx.subscribe();
        tokio::select! {
            _ = check_rx.recv() => break,
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {}
        }
    }

    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}
