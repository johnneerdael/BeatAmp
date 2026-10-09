use audio_analysis::{Analyzer, Snapshot};
use crossbeam_channel::{Receiver, Sender, bounded};
use serde::Serialize;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const SAMPLES: usize = 512;
const QUEUE_BLOCKS: usize = 8;

struct Block {
    samples: Box<[i16; SAMPLES]>,
    length: usize,
    start_frame: u64,
    end_ns: u64,
    estimated: bool,
    enqueued: Instant,
}

struct Counters {
    accepted: AtomicU64,
    dropped: AtomicU64,
    rejected: AtomicU64,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct WorkerReport {
    pub snapshot: Option<Snapshot>,
    pub accepted_blocks: u64,
    pub processed_blocks: u64,
    pub dropped_blocks: u64,
    pub rejected_blocks: u64,
    pub queue_stale_blocks: u64,
    pub analysis_us_max: u64,
    pub analysis_us_mean: f64,
    pub worker_cpu_ns: Option<u64>,
    pub worker_wall_ns: u64,
    pub error: Option<String>,
    pub waveform: Vec<[f32; 2]>,
}

pub struct AudioWorker {
    channels: usize,
    pending: Sender<Block>,
    free: Receiver<Box<[i16; SAMPLES]>>,
    recycle: Sender<Box<[i16; SAMPLES]>>,
    counters: Arc<Counters>,
    latest: Arc<Mutex<WorkerReport>>,
    stopping: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}
impl AudioWorker {
    pub fn new(rate: u32, channels: u16, generation: u64) -> Result<Self, String> {
        let analyzer = Analyzer::new(rate, channels, generation).map_err(|e| e.to_string())?;
        let (pending, incoming) = bounded::<Block>(QUEUE_BLOCKS);
        let (recycle, free) = bounded(QUEUE_BLOCKS + 2);
        for _ in 0..QUEUE_BLOCKS + 2 {
            recycle
                .try_send(Box::new([0; SAMPLES]))
                .map_err(|e| e.to_string())?;
        }
        let counters = Arc::new(Counters {
            accepted: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
        });
        let latest = Arc::new(Mutex::new(WorkerReport::default()));
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_latest = latest.clone();
        let worker_stop = stopping.clone();
        let worker_recycle = recycle.clone();
        let join = thread::Builder::new()
            .name("pcm-analysis".into())
            .spawn(move || {
                analyze_worker(
                    analyzer,
                    incoming,
                    worker_recycle,
                    worker_latest,
                    worker_stop,
                    channels as usize,
                );
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            channels: channels as usize,
            pending,
            free,
            recycle,
            counters,
            latest,
            stopping,
            join: Some(join),
        })
    }

    /// Callback path: preallocated copy and nonblocking channel operations only.
    pub fn submit(&self, pcm: &[i16], start_frame: u64, end_ns: u64, estimated: bool) -> bool {
        if pcm.is_empty() || pcm.len() > SAMPLES || !pcm.len().is_multiple_of(self.channels) {
            self.counters.rejected.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        let Ok(mut samples) = self.free.try_recv() else {
            self.counters.dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        };
        samples[..pcm.len()].copy_from_slice(pcm);
        let block = Block {
            samples,
            length: pcm.len(),
            start_frame,
            end_ns,
            estimated,
            enqueued: Instant::now(),
        };
        match self.pending.try_send(block) {
            Ok(()) => {
                self.counters.accepted.fetch_add(1, Ordering::Relaxed);
                true
            }
            Err(e) => {
                let _ = self.recycle.try_send(e.into_inner().samples);
                self.counters.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    pub fn report(&self) -> WorkerReport {
        let mut r = self
            .latest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        r.accepted_blocks = self.counters.accepted.load(Ordering::Relaxed);
        r.dropped_blocks = self.counters.dropped.load(Ordering::Relaxed);
        r.rejected_blocks = self.counters.rejected.load(Ordering::Relaxed);
        r
    }
}

impl Drop for AudioWorker {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn analyze_worker(
    mut analyzer: Analyzer,
    incoming: Receiver<Block>,
    recycle: Sender<Box<[i16; SAMPLES]>>,
    latest: Arc<Mutex<WorkerReport>>,
    stopping: Arc<AtomicBool>,
    channels: usize,
) {
    let started = Instant::now();
    let cpu_start = thread_cpu_ns();
    let mut report = WorkerReport::default();
    let mut analysis_ns = 0u64;
    let mut wave = [[0.0f32; 2]; 512];
    let mut wave_at = 0usize;
    let mut expected_wave_frame = None;
    while !stopping.load(Ordering::Acquire) {
        let Ok(block) = incoming.recv_timeout(Duration::from_millis(10)) else {
            continue;
        };
        if block.enqueued.elapsed() > Duration::from_millis(40) {
            report.queue_stale_blocks += 1;
        } else {
            let before = Instant::now();
            if expected_wave_frame.is_some_and(|f| f != block.start_frame) {
                wave.fill([0.0; 2]);
                wave_at = 0;
            }
            expected_wave_frame = Some(block.start_frame + (block.length / channels) as u64);
            for frame in block.samples[..block.length].chunks_exact(channels) {
                wave[wave_at] = [
                    frame[0] as f32 / 32768.0,
                    frame.get(1).copied().unwrap_or(0) as f32 / 32768.0,
                ];
                wave_at = (wave_at + 1) % 512;
            }
            report.waveform = (0..512).map(|i| wave[(wave_at + i) % 512]).collect();
            match analyzer.push_i16(
                &block.samples[..block.length],
                block.start_frame,
                block.end_ns,
                block.estimated,
            ) {
                Ok(Some(snapshot)) => {
                    report.snapshot = Some(snapshot);
                }
                Ok(None) => {}
                Err(e) => {
                    report.error = Some(e.to_string());
                }
            }
            let ns = before.elapsed().as_nanos().min(u64::MAX as u128) as u64;
            analysis_ns += ns;
            report.processed_blocks += 1;
            report.analysis_us_max = report.analysis_us_max.max(ns / 1000);
            report.analysis_us_mean = analysis_ns as f64 / report.processed_blocks as f64 / 1000.0;
        }
        report.worker_wall_ns = started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        report.worker_cpu_ns = thread_cpu_ns()
            .zip(cpu_start)
            .map(|(now, begin)| now.saturating_sub(begin));
        if let Ok(mut output) = latest.try_lock() {
            *output = report.clone();
        }
        let _ = recycle.try_send(block.samples);
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn thread_cpu_ns() -> Option<u64> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: valid writable timespec; no borrowed data escapes this call.
    (unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) } == 0)
        .then(|| (time.tv_sec as u64) * 1_000_000_000 + time.tv_nsec as u64)
}
#[cfg(not(any(target_os = "android", target_os = "linux")))]
fn thread_cpu_ns() -> Option<u64> {
    None
}

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
mod beat_player;
#[cfg(target_os = "android")]
mod render;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        thread,
        time::{Duration, Instant},
    };

    #[test]
    fn worker_preserves_generation_and_reports_measured_work() {
        let w = AudioWorker::new(48000, 2, 19).unwrap();
        assert!(w.submit(&[12000; 512], 0, 1_000_000_000, false));
        let deadline = Instant::now() + Duration::from_secs(2);
        while w.report().processed_blocks == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        let r = w.report();
        assert_eq!(r.processed_blocks, 1);
        assert_eq!(r.snapshot.unwrap().generation, 19);
        assert!(r.worker_wall_ns > 0);
    }

    #[test]
    fn pcm_window_preserves_stereo_and_resets_on_gaps() {
        let w = AudioWorker::new(48000, 2, 1).unwrap();
        let pcm: Vec<i16> = (0..256).flat_map(|_| [12000, -6000]).collect();
        assert!(w.submit(&pcm, 0, 1_000_000_000, false));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while w.report().processed_blocks < 1 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let r = w.report();
        assert_eq!(r.waveform.len(), 512);
        assert_eq!(r.waveform[511], [12000.0 / 32768.0, -6000.0 / 32768.0]);
        assert!(w.submit(&[0; 512], 10000, 2_000_000_000, false));
        while w.report().processed_blocks < 2 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(w.report().waveform.iter().all(|p| *p == [0.0; 2]));
    }

    #[test]
    fn malformed_and_oversized_blocks_are_rejected_before_enqueue() {
        let w = AudioWorker::new(48000, 2, 1).unwrap();
        assert!(!w.submit(&[0; 513], 0, 0, true));
        assert!(!w.submit(&[], 0, 0, true));
        assert!(!w.submit(&[0; 3], 0, 0, true));
        assert_eq!(w.report().rejected_blocks, 3);
    }

    #[test]
    fn invalid_source_format_cannot_start_a_worker() {
        assert!(AudioWorker::new(0, 2, 1).is_err());
        assert!(AudioWorker::new(48000, 5, 1).is_err());
    }
}
