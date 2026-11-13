use std::{
    any::Any,
    cell::UnsafeCell,
    collections::VecDeque,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use super::{FfiBatch, FfiJob};

type PanicPayload = Box<dyn Any + Send>;

pub(super) struct ThreadPoolNative {
    shared: Arc<Shared>,
    frame_threads: Vec<JoinHandle<()>>,
    background_threads: Vec<JoinHandle<()>>,
}

impl ThreadPoolNative {
    pub fn new(frame_threads_count: usize, background_threads_count: usize) -> Self {
        assert!(frame_threads_count > 0);
        assert!(background_threads_count > 0);

        let shared = Arc::new(Shared {
            frame: Mutex::new(FrameState {
                queue: VecDeque::new(),
                terminate: false,
            }),
            frame_cv: Condvar::new(),
            background: Mutex::new(BackgroundState {
                queue: VecDeque::new(),
                terminate: false,
            }),
            background_cv: Condvar::new(),
            did_panic: AtomicBool::new(false),
        });

        let frame_threads = (0..frame_threads_count)
            .map(|_| {
                let shared = Arc::clone(&shared);
                thread::spawn(move || shared.run_frame_worker())
            })
            .collect();

        let background_threads = (0..background_threads_count)
            .map(|_| {
                let shared = Arc::clone(&shared);
                thread::spawn(move || shared.run_background_worker())
            })
            .collect();

        Self {
            shared,
            frame_threads,
            background_threads,
        }
    }

    pub fn threads_count(&self) -> usize {
        self.frame_threads.len()
    }

    pub fn background_threads_count(&self) -> usize {
        self.background_threads.len()
    }

    pub fn did_panic(&self) -> bool {
        self.shared.did_panic.load(Ordering::Relaxed)
    }

    pub fn execute_batch(&self, batch: FfiBatch) {
        self.shared.execute_batch(batch);
    }

    pub fn push_frame_job(&self, job: FfiJob) {
        lock(&self.shared.frame).queue.push_back(FrameEntry::Job(job));
        self.shared.frame_cv.notify_one();
    }

    pub fn push_background_job(&self, job: FfiJob) {
        lock(&self.shared.background).queue.push_back(job);
        self.shared.background_cv.notify_one();
    }
}

impl Drop for ThreadPoolNative {
    fn drop(&mut self) {
        lock(&self.shared.frame).terminate = true;
        lock(&self.shared.background).terminate = true;
        self.shared.frame_cv.notify_all();
        self.shared.background_cv.notify_all();

        for join_handle in self.frame_threads.drain(..).chain(self.background_threads.drain(..)) {
            join_handle.join().unwrap();
        }

        if !thread::panicking() && self.did_panic() {
            panic!("ThreadPool worker thread panicked");
        }
    }
}

//

struct Shared {
    frame: Mutex<FrameState>,
    frame_cv: Condvar,
    background: Mutex<BackgroundState>,
    background_cv: Condvar,
    did_panic: AtomicBool,
}

struct FrameState {
    queue: VecDeque<FrameEntry>,
    terminate: bool,
}

enum FrameEntry {
    Task(BatchPtr, u64),
    Job(FfiJob),
}

struct BackgroundState {
    queue: VecDeque<FfiJob>,
    terminate: bool,
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct BatchPtr(*const BatchState);

unsafe impl Send for BatchPtr {}

// Lives on the caller's stack; `execute_batch` returns only once `running` and `queued` are both 0,
// so no worker can still reference it.
struct BatchState {
    batch: FfiBatch,
    iter_lock: Mutex<()>,
    poisoned: AtomicBool,
    // Guarded by `Shared::frame`.
    counters: UnsafeCell<BatchCounters>,
    cv: Condvar,
}

struct BatchCounters {
    running: usize,
    queued: usize,
    panic: Option<PanicPayload>,
}

unsafe impl Sync for BatchState {}

impl BatchState {
    unsafe fn counters<'a>(&'a self, _frame: &'a mut FrameState) -> &'a mut BatchCounters {
        unsafe { &mut *self.counters.get() }
    }

    fn ptr(&self) -> BatchPtr {
        BatchPtr(self)
    }
}

impl Shared {
    fn run_frame_worker(&self) {
        loop {
            let entry = {
                let mut frame = lock(&self.frame);

                loop {
                    if let Some(entry) = frame.queue.pop_front() {
                        if let FrameEntry::Task(batch, _) = &entry {
                            let counters = unsafe { (&*batch.0).counters(&mut frame) };
                            counters.queued -= 1;
                            counters.running += 1;
                        }

                        break entry;
                    }

                    if frame.terminate {
                        return;
                    }

                    frame = self.frame_cv.wait(frame).unwrap_or_else(|e| e.into_inner());
                }
            };

            match entry {
                FrameEntry::Task(batch, task) => self.run_task_chain(unsafe { &*batch.0 }, task),
                FrameEntry::Job(job) => self.run_job(job),
            }
        }
    }

    fn run_background_worker(&self) {
        loop {
            let job = {
                let mut background = lock(&self.background);

                loop {
                    if let Some(job) = background.queue.pop_front() {
                        break job;
                    }

                    if background.terminate {
                        return;
                    }

                    background = self.background_cv.wait(background).unwrap_or_else(|e| e.into_inner());
                }
            };

            self.run_job(job);
        }
    }

    fn run_job(&self, job: FfiJob) {
        if catch_unwind(AssertUnwindSafe(|| job.call())).is_err() {
            self.did_panic.store(true, Ordering::Relaxed);
        }
    }

    fn execute_batch(&self, batch: FfiBatch) {
        let state = BatchState {
            batch,
            iter_lock: Mutex::new(()),
            poisoned: AtomicBool::new(false),
            counters: UnsafeCell::new(BatchCounters {
                running: 0,
                queued: 0,
                panic: None,
            }),
            cv: Condvar::new(),
        };

        let mut ready = Vec::new();
        self.pull_ready(&state, None, &mut ready);
        let mut own_task = self.publish(&state, false, &mut ready);

        loop {
            if let Some(task) = own_task.take() {
                self.run_task_chain(&state, task);
            }

            let mut frame = lock(&self.frame);

            loop {
                let position = frame
                    .queue
                    .iter()
                    .position(|e| matches!(e, FrameEntry::Task(b, _) if *b == state.ptr()));

                if let Some(position) = position {
                    let Some(FrameEntry::Task(_, task)) = frame.queue.remove(position) else {
                        unreachable!();
                    };

                    let counters = unsafe { state.counters(&mut frame) };
                    counters.queued -= 1;
                    counters.running += 1;
                    own_task = Some(task);
                    break;
                }

                let counters = unsafe { state.counters(&mut frame) };

                if counters.running == 0 && counters.queued == 0 {
                    let panic = counters.panic.take();
                    drop(frame);

                    if let Some(panic) = panic {
                        resume_unwind(panic);
                    }

                    return;
                }

                frame = state.cv.wait(frame).unwrap_or_else(|e| e.into_inner());
            }
        }
    }

    // The calling thread holds one `running` slot of `batch` for `task`. After the last `publish`
    // returns `None` the batch may be gone, so it is not touched again.
    fn run_task_chain(&self, batch: &BatchState, mut task: u64) {
        let mut ready = Vec::new();

        loop {
            match catch_unwind(AssertUnwindSafe(|| unsafe { batch.batch.execute(task) })) {
                Ok(()) => self.pull_ready(batch, Some(task), &mut ready),
                Err(panic) => self.poison(batch, panic),
            }

            match self.publish(batch, true, &mut ready) {
                Some(next_task) => task = next_task,
                None => return,
            }
        }
    }

    fn pull_ready(&self, batch: &BatchState, finished: Option<u64>, ready: &mut Vec<u64>) {
        if batch.poisoned.load(Ordering::Acquire) {
            return;
        }

        let _iter_guard = lock(&batch.iter_lock);

        if batch.poisoned.load(Ordering::Acquire) {
            return;
        }

        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut finished = finished;

            while let Some(task) = unsafe { batch.batch.next(finished.take()) } {
                ready.push(task);
            }
        }));

        if let Err(panic) = result {
            self.poison(batch, panic);
        }
    }

    fn poison(&self, batch: &BatchState, panic: PanicPayload) {
        let mut frame = lock(&self.frame);

        batch.poisoned.store(true, Ordering::Release);

        frame
            .queue
            .retain(|e| !matches!(e, FrameEntry::Task(b, _) if *b == batch.ptr()));

        let counters = unsafe { batch.counters(&mut frame) };
        counters.queued = 0;
        counters.panic.get_or_insert(panic);

        batch.cv.notify_all();
    }

    fn publish(&self, batch: &BatchState, completed: bool, ready: &mut Vec<u64>) -> Option<u64> {
        let mut frame = lock(&self.frame);

        let poisoned = batch.poisoned.load(Ordering::Acquire);

        if poisoned {
            ready.clear();
        }

        let mut ready = ready.drain(..);
        let own_task = ready.next();
        let mut queued_count = 0;

        for task in ready {
            frame.queue.push_back(FrameEntry::Task(batch.ptr(), task));
            queued_count += 1;
        }

        let counters = unsafe { batch.counters(&mut frame) };

        if completed {
            counters.running -= 1;
        }

        if own_task.is_some() {
            counters.running += 1;
        }

        counters.queued += queued_count;

        match queued_count {
            0 => {}
            1 => self.frame_cv.notify_one(),
            _ => self.frame_cv.notify_all(),
        }

        if queued_count > 0 || (counters.running == 0 && counters.queued == 0) {
            batch.cv.notify_all();
        }

        own_task
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
