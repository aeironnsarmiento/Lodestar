//! Per-process CPU and RAM sampling (R12).

use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMetrics {
    /// Share of the whole machine, 0–100.
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}

pub struct Sampler {
    system: System,
    cpus: f32,
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Sampler {
    pub fn new() -> Self {
        let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f32;
        Self { system: System::new(), cpus }
    }

    /// Samples the given processes. CPU is measured since the previous call, so the
    /// first sample of a new process reads 0.
    pub fn sample(&mut self, pids: &[u32]) -> Vec<(u32, ProcessMetrics)> {
        let list: Vec<Pid> = pids.iter().map(|&p| Pid::from_u32(p)).collect();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&list),
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        pids.iter()
            .filter_map(|&pid| {
                let p = self.system.process(Pid::from_u32(pid))?;
                Some((
                    pid,
                    ProcessMetrics {
                        cpu_percent: (p.cpu_usage() / self.cpus).clamp(0.0, 100.0),
                        memory_bytes: p.memory(),
                    },
                ))
            })
            .collect()
    }
}
