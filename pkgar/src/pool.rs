use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
};

type Job = Box<dyn FnOnce() + Send + 'static>;

enum BatchMsg<S, E> {
    Progress(usize),
    Done(S, Option<E>),
}

pub struct WorkerPool {
    tx: Option<mpsc::Sender<Job>>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl WorkerPool {
    pub fn new(threads: usize) -> Self {
        if threads <= 1 {
            return Self {
                tx: None,
                workers: Vec::new(),
            };
        }

        let (tx, rx) = mpsc::channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        let mut workers = Vec::with_capacity(threads);

        for _ in 0..threads {
            let rx = Arc::clone(&rx);
            workers.push(thread::spawn(move || {
                loop {
                    let job = {
                        let Ok(guard) = rx.lock() else { break };
                        guard.recv()
                    };

                    match job {
                        Ok(job) => job(),
                        Err(_) => break,
                    }
                }
            }));
        }

        Self {
            tx: Some(tx),
            workers,
        }
    }

    pub fn execute(&self, job: impl FnOnce() + Send + 'static) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Box::new(job));
        }
    }

    /// Distributes `items` across `worker_states`.
    pub fn run_batch<T, S, E, Work, OnProgress>(
        &self,
        items: Vec<T>,
        worker_states: Vec<S>,
        work: Work,
        mut on_progress: OnProgress,
    ) -> (Vec<S>, Option<E>)
    where
        T: Send + Sync + 'static,
        S: Send + 'static,
        E: Send + 'static,
        Work: Fn(&mut S, &T) -> Result<usize, E> + Send + Sync + 'static,
        OnProgress: FnMut(usize),
    {
        if self.tx.is_none() {
            let mut states = worker_states;
            let mut first_err = None;
            if let Some(state) = states.first_mut() {
                for item in &items {
                    match work(state, item) {
                        Ok(delta) if delta > 0 => on_progress(delta),
                        Ok(_) => {}
                        Err(e) => {
                            first_err = Some(e);
                            break;
                        }
                    }
                }
            }
            return (states, first_err);
        }

        let num_workers = worker_states.len();
        let items = Arc::new(items);
        let work = Arc::new(work);
        let next_idx = Arc::new(AtomicUsize::new(0));
        let aborted = Arc::new(AtomicBool::new(false));
        let (msg_tx, msg_rx) = mpsc::channel::<BatchMsg<S, E>>();

        for mut state in worker_states {
            let items = Arc::clone(&items);
            let work = Arc::clone(&work);
            let next_idx = Arc::clone(&next_idx);
            let aborted = Arc::clone(&aborted);
            let msg_tx = msg_tx.clone();

            self.execute(move || {
                let mut err = None;
                loop {
                    if aborted.load(Ordering::Relaxed) {
                        break;
                    }
                    let idx = next_idx.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(idx) else {
                        break;
                    };

                    match work(&mut state, item) {
                        Ok(delta) => {
                            if delta > 0 {
                                let _ = msg_tx.send(BatchMsg::Progress(delta));
                            }
                        }
                        Err(e) => {
                            aborted.store(true, Ordering::Relaxed);
                            err = Some(e);
                            break;
                        }
                    }
                }
                let _ = msg_tx.send(BatchMsg::Done(state, err));
            });
        }

        drop(msg_tx);

        let mut states = Vec::with_capacity(num_workers);
        let mut first_err = None;
        for msg in msg_rx {
            match msg {
                BatchMsg::Progress(delta) => on_progress(delta),
                BatchMsg::Done(state, err) => {
                    states.push(state);
                    if first_err.is_none() {
                        first_err = err;
                    }
                }
            }
        }

        (states, first_err)
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.tx.take();
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
    }
}
