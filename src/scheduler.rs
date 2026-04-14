use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use crate::key_sender::{VirtualKey, send_key};

#[derive(Clone, Debug)]
pub struct KeyTask {
    pub id: u32,
    pub vk: VirtualKey,
    pub interval_ms: u64,
}

#[derive(Clone, Debug, Default)]
pub struct SendStats {
    pub counts: HashMap<u32, u64>,
}

pub struct Scheduler {
    target_hwnd: isize,
    tasks: Vec<KeyTask>,
    running: Arc<AtomicBool>,
    stats: Arc<Mutex<SendStats>>,
    handles: Vec<thread::JoinHandle<()>>,
}

impl Scheduler {
    pub fn new(target_hwnd: isize, tasks: Vec<KeyTask>) -> Self {
        Self {
            target_hwnd,
            tasks,
            running: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(Mutex::new(SendStats::default())),
            handles: Vec::new(),
        }
    }

    pub fn start(&mut self) {
        if self.running.load(Ordering::SeqCst) {
            return;
        }
        self.running.store(true, Ordering::SeqCst);

        {
            let mut stats = self.stats.lock().unwrap();
            stats.counts.clear();
        }

        for task in &self.tasks {
            let running = Arc::clone(&self.running);
            let stats = Arc::clone(&self.stats);
            let hwnd = self.target_hwnd;
            let task = task.clone();

            let handle = thread::spawn(move || {
                let interval = Duration::from_millis(task.interval_ms);
                while running.load(Ordering::SeqCst) {
                    let start = Instant::now();
                    send_key(hwnd, task.vk);

                    {
                        let mut s = stats.lock().unwrap();
                        *s.counts.entry(task.id).or_insert(0) += 1;
                    }

                    let elapsed = start.elapsed();
                    if elapsed < interval {
                        let remaining = interval - elapsed;
                        let sleep_step = Duration::from_millis(50);
                        let mut slept = Duration::ZERO;
                        while slept < remaining && running.load(Ordering::SeqCst) {
                            let step = sleep_step.min(remaining - slept);
                            thread::sleep(step);
                            slept += step;
                        }
                    }
                }
            });
            self.handles.push(handle);
        }
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn stats(&self) -> SendStats {
        self.stats.lock().unwrap().clone()
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.stop();
    }
}
