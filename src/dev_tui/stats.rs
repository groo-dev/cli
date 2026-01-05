use std::collections::HashMap;
use std::time::Instant;
use sysinfo::{Pid, ProcessesToUpdate, System};

/// Resource stats for a process
#[derive(Debug, Clone, Default)]
pub struct ResourceStats {
    pub cpu_percent: f32,
    pub memory_mb: u64,
    #[allow(dead_code)]
    pub last_updated: Option<Instant>,
}

/// Collector for process stats
pub struct StatsCollector {
    system: System,
    stats: HashMap<u32, ResourceStats>,
    last_refresh: Option<Instant>,
    refresh_interval: std::time::Duration,
}

impl StatsCollector {
    pub fn new() -> Self {
        Self {
            system: System::new(),
            stats: HashMap::new(),
            last_refresh: None,
            refresh_interval: std::time::Duration::from_secs(2),
        }
    }

    /// Check if stats should be refreshed
    pub fn should_refresh(&self) -> bool {
        self.last_refresh
            .map(|t| t.elapsed() >= self.refresh_interval)
            .unwrap_or(true)
    }

    /// Refresh stats for given PIDs
    pub fn refresh(&mut self, pids: &[u32]) {
        // Convert PIDs to sysinfo format
        let sysinfo_pids: Vec<Pid> = pids.iter().map(|&p| Pid::from_u32(p)).collect();

        // Refresh process info for specific PIDs
        self.system.refresh_processes(ProcessesToUpdate::Some(&sysinfo_pids), true);

        // Collect stats
        for &pid in pids {
            if let Some(process) = self.system.process(Pid::from_u32(pid)) {
                self.stats.insert(
                    pid,
                    ResourceStats {
                        cpu_percent: process.cpu_usage(),
                        memory_mb: process.memory() / 1024 / 1024,
                        last_updated: Some(Instant::now()),
                    },
                );
            }
        }

        self.last_refresh = Some(Instant::now());
    }

    /// Get stats for a PID
    #[allow(dead_code)]
    pub fn get(&self, pid: u32) -> Option<&ResourceStats> {
        self.stats.get(&pid)
    }

    /// Get aggregate stats (total CPU and memory)
    pub fn aggregate(&self) -> ResourceStats {
        let mut total = ResourceStats::default();
        for stats in self.stats.values() {
            total.cpu_percent += stats.cpu_percent;
            total.memory_mb += stats.memory_mb;
        }
        total
    }

    /// Remove stats for a PID
    pub fn remove(&mut self, pid: u32) {
        self.stats.remove(&pid);
    }
}

impl Default for StatsCollector {
    fn default() -> Self {
        Self::new()
    }
}
