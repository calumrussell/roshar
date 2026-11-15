use std::sync::atomic::{AtomicU64, Ordering};

/// Strictly increasing millisecond nonces: `max(now_ms, last + 1)`. Thread-safe and lock-free.
#[derive(Debug, Default)]
pub struct NonceGen {
    last: AtomicU64,
}

impl NonceGen {
    pub fn new() -> Self {
        NonceGen {
            last: AtomicU64::new(0),
        }
    }

    pub fn next(&self) -> u64 {
        let now = crate::now_ns() / 1_000_000;
        let mut last = self.last.load(Ordering::Relaxed);
        loop {
            let n = now.max(last + 1);
            match self
                .last
                .compare_exchange_weak(last, n, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => return n,
                Err(cur) => last = cur,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn monotonic_and_unique_across_threads() {
        let g = Arc::new(NonceGen::new());
        let hs: Vec<_> = (0..4)
            .map(|_| {
                let g = g.clone();
                std::thread::spawn(move || (0..10_000).map(|_| g.next()).collect::<Vec<_>>())
            })
            .collect();
        let mut all = Vec::new();
        for h in hs {
            let v = h.join().unwrap();
            assert!(v.windows(2).all(|w| w[1] > w[0]));
            all.extend(v);
        }
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n);
        assert!(all[0] >= 1_700_000_000_000);
    }
}
