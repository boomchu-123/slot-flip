use std::collections::VecDeque;

/// The history rewind may keep. The Mini Plus has about 100 MB in all and the
/// kernel killed the frontend at 74 MB with the larger budget, a few minutes
/// into a Super Nintendo game; the smaller one keeps a few seconds of history.
#[cfg(feature = "miyoomini")]
pub const REWIND_BYTES: usize = 4 * 1024 * 1024;
#[cfg(not(feature = "miyoomini"))]
pub const REWIND_BYTES: usize = 20 * 1024 * 1024;

pub struct Rewind {
    budget: usize,
    cur: Option<Vec<u8>>,
    deltas: VecDeque<Vec<u8>>,
    bytes: usize,
    scratch: Vec<u8>,
}

impl Rewind {
    pub fn new(budget_bytes: usize) -> Self {
        Rewind {
            budget: budget_bytes,
            cur: None,
            deltas: VecDeque::new(),
            bytes: 0,
            scratch: Vec::new(),
        }
    }

    pub fn push(&mut self, state: &[u8]) {
        let Some(prev) = self.cur.replace(state.to_vec()) else {
            return;
        };
        if prev.len() != state.len() {
            self.deltas.clear();
            self.bytes = 0;
            return;
        }
        self.scratch.clear();
        self.scratch
            .extend(prev.iter().zip(state).map(|(a, b)| a ^ b));
        let entry = lz4_flex::compress(&self.scratch);
        self.bytes += entry.len();
        self.deltas.push_back(entry);
        while self.bytes > self.budget {
            match self.deltas.pop_front() {
                Some(oldest) => self.bytes -= oldest.len(),
                None => break,
            }
        }
    }

    pub fn pop(&mut self) -> Option<Vec<u8>> {
        let cur = self.cur.take()?;
        let Some(entry) = self.deltas.pop_back() else {
            return Some(cur);
        };
        self.bytes -= entry.len();
        let mut prev = vec![0u8; cur.len()];
        match lz4_flex::decompress_into(&entry, &mut prev) {
            Ok(_) => {
                for (p, c) in prev.iter_mut().zip(&cur) {
                    *p ^= c;
                }
                self.cur = Some(prev);
            }
            Err(e) => {
                eprintln!("slot: rewind: {e}");
                self.deltas.clear();
                self.bytes = 0;
            }
        }
        Some(cur)
    }

    pub fn bytes_used(&self) -> usize {
        self.bytes
    }

    pub fn depth(&self) -> usize {
        self.deltas.len() + usize::from(self.cur.is_some())
    }

    pub fn fill(&self) -> u8 {
        if self.budget == 0 {
            return 0;
        }
        (self.bytes * 100 / self.budget).min(100) as u8
    }
}

pub struct RewindThread {
    tx: std::sync::mpsc::SyncSender<Msg>,
    fill: std::sync::Arc<std::sync::atomic::AtomicU8>,
}

enum Msg {
    Push(Vec<u8>),
    Pop(std::sync::mpsc::SyncSender<Option<Vec<u8>>>),
}

impl RewindThread {
    pub fn spawn(budget_bytes: usize) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Msg>(4);
        let fill = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0));
        let published = fill.clone();
        std::thread::Builder::new()
            .name("slot-rewind".into())
            .spawn(move || {
                let mut rewind = Rewind::new(budget_bytes);
                while let Ok(msg) = rx.recv() {
                    match msg {
                        Msg::Push(state) => {
                            rewind.push(&state);
                            published.store(rewind.fill(), std::sync::atomic::Ordering::Relaxed);
                        }
                        Msg::Pop(reply) => {
                            let out = rewind.pop();
                            published.store(rewind.fill(), std::sync::atomic::Ordering::Relaxed);
                            let _ = reply.send(out);
                        }
                    }
                }
            })
            .expect("rewind thread");
        RewindThread { tx, fill }
    }

    pub fn push(&self, state: Vec<u8>) {
        let _ = self.tx.send(Msg::Push(state));
    }

    pub fn pop(&self) -> Option<Vec<u8>> {
        let (tx, rx) = std::sync::mpsc::sync_channel(0);
        if self.tx.send(Msg::Pop(tx)).is_err() {
            return None;
        }
        rx.recv().ok().flatten()
    }

    pub fn fill(&self) -> u8 {
        self.fill.load(std::sync::atomic::Ordering::Relaxed)
    }
}
