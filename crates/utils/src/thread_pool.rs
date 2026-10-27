use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::AtomicBool,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
};

pub use job::*;

use crate::exec_on_drop::ExecOnDrop;

// todo: ffi?

type Job = Box<dyn FnOnce() + Send>;

enum Message {
    JobRequestUnhandled(Job),
    JobRequestHandled(JobExecutor<Job>),
    TerminateRequest,
}

struct Worker {
    pub _id: usize,
    pub message_receiver: Arc<Mutex<Receiver<Message>>>,
    pub did_panic: Arc<AtomicBool>,
}

impl Worker {
    fn run(self) -> JoinHandle<()> {
        thread::spawn(move || {
            let exec_on_drop = ExecOnDrop::new(|| {
                if std::thread::panicking() {
                    self.did_panic.store(true, std::sync::atomic::Ordering::Relaxed)
                }
            });

            loop {
                let message = { self.message_receiver.lock().unwrap().recv().unwrap() };

                match message {
                    Message::TerminateRequest => break,
                    Message::JobRequestUnhandled(job) => job(),
                    Message::JobRequestHandled(job) => job.execute(),
                }
            }

            drop(exec_on_drop);
        })
    }
}

pub struct ThreadPool {
    threads: Vec<JoinHandle<()>>,
    message_sender: Sender<Message>,
    did_panic: Arc<AtomicBool>,
}

impl ThreadPool {
    pub fn new(threads_count: usize) -> Self {
        assert!(threads_count > 0);

        let mut threads = Vec::with_capacity(threads_count);

        let (message_sender, message_receiver) = mpsc::channel();

        let message_receiver = Arc::new(Mutex::new(message_receiver));
        let did_panic = Arc::new(AtomicBool::new(false));

        for id in 0..threads_count {
            let worker = Worker {
                _id: id,
                message_receiver: Arc::clone(&message_receiver),
                did_panic: Arc::clone(&did_panic),
            };

            threads.push(worker.run());
        }

        Self {
            threads,
            message_sender,
            did_panic,
        }
    }

    pub fn threads_count(&self) -> usize {
        self.threads.len()
    }

    pub fn push_job_unhandled<F: 'static + Send + FnOnce() -> T, T>(&self, f: F) {
        let job = Box::new(move || _ = f());

        self.message_sender.send(Message::JobRequestUnhandled(job)).unwrap();
    }

    pub fn push_job_handled<F: 'static + Send + FnOnce() -> Job>(&self, f: F) -> JobHandle<Job> {
        let (job_handle, job_executor) = create_job(f);

        self.message_sender.send(Message::JobRequestHandled(job_executor)).unwrap();

        job_handle
    }

    pub fn scope<F: for<'scope> FnOnce(&'scope scope::Scope<'scope>) -> T, T>(&self, f: F) -> T {
        scope::scope(self, f)
    }

    pub fn panic_if_err(&self) {
        let did_panic = self.did_panic.load(std::sync::atomic::Ordering::Relaxed);

        if did_panic {
            panic!("ThreadPool worker thread panicked");
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        for _ in 0..self.threads.len() {
            self.message_sender.send(Message::TerminateRequest).unwrap();
        }

        while let Some(join_handle) = self.threads.pop() {
            join_handle.join().unwrap();
        }

        if !std::thread::panicking() {
            self.panic_if_err();
        }
    }
}

mod job {
    use std::{
        cell::UnsafeCell,
        mem::MaybeUninit,
        sync::{Arc, atomic::AtomicU8},
    };

    pub fn create_job<F: 'static + Send + FnOnce() -> T, T: 'static + Send>(f: F) -> (JobHandle<T>, JobExecutor<T>) {
        let state = Arc::new(JobState {
            state: AtomicU8::new(0),
            f: UnsafeCell::new(MaybeUninit::new(f)),
            result: UnsafeCell::new(MaybeUninit::uninit()),
        }) as Arc<dyn 'static + Send + Sync + AbstractJobState<T>>;

        (JobHandle { state: state.clone() }, JobExecutor { state })
    }

    pub struct JobHandle<T: 'static + Send> {
        state: Arc<dyn 'static + Send + Sync + AbstractJobState<T>>,
    }

    impl<T: 'static + Send> JobHandle<T> {
        pub fn is_finished(&self) -> bool {
            self.state.is_finished()
        }

        pub fn try_take(self) -> Result<T, Self> {
            if self.state.is_finished() {
                Ok(self.state.try_take_result().unwrap())
            } else {
                Err(self)
            }
        }

        pub fn block_and_take(self) -> T {
            // todo: CondVar
            while !self.state.is_finished() {}

            self.state.try_take_result().unwrap()
        }
    }

    pub struct JobExecutor<T: 'static + Send> {
        state: Arc<dyn 'static + Send + Sync + AbstractJobState<T>>,
    }

    impl<T: 'static + Send> JobExecutor<T> {
        pub fn execute(self) {
            self.state.try_execute();
        }
    }

    trait AbstractJobState<T: 'static + Send> {
        fn try_execute(&self);
        fn is_finished(&self) -> bool;
        fn try_take_result(&self) -> Option<T>;
    }

    struct JobState<F: 'static + Send + FnOnce() -> T, T: 'static + Send> {
        state: AtomicU8,
        f: UnsafeCell<MaybeUninit<F>>,
        result: UnsafeCell<MaybeUninit<T>>,
    }

    unsafe impl<F: 'static + Send + FnOnce() -> T, T: 'static + Send> Sync for JobState<F, T> {}

    impl<F: 'static + Send + FnOnce() -> T, T: 'static + Send> AbstractJobState<T> for JobState<F, T> {
        fn try_execute(&self) {
            let f = unsafe {
                if self.state.fetch_max(1, std::sync::atomic::Ordering::AcqRel) != 0 {
                    return;
                }

                self.f.get().read().assume_init()
            };

            let result = f();

            unsafe {
                self.result.get().write(MaybeUninit::new(result));
                self.state.store(2, std::sync::atomic::Ordering::Release);
            }
        }

        fn is_finished(&self) -> bool {
            self.state.load(std::sync::atomic::Ordering::Acquire) >= 2
        }

        fn try_take_result(&self) -> Option<T> {
            Some(unsafe {
                if self.state.fetch_max(3, std::sync::atomic::Ordering::AcqRel) != 2 {
                    return None;
                }

                self.result.get().read().assume_init()
            })
        }
    }

    impl<F: 'static + Send + FnOnce() -> T, T: 'static + Send> Drop for JobState<F, T> {
        fn drop(&mut self) {
            unsafe {
                let state = self.state.load(std::sync::atomic::Ordering::Acquire);

                match state {
                    0 => self.f.get().drop_in_place(),
                    2 => self.result.get().drop_in_place(),
                    _ => (),
                }
            }
        }
    }
}

mod scope {
    use std::sync::{Arc, atomic::AtomicUsize};

    use crate::{exec_on_drop::ExecOnDrop, thread_pool::ThreadPool};

    pub fn scope<F: for<'scope> FnOnce(&'scope Scope<'scope>) -> T, T>(thread_pool: &ThreadPool, f: F) -> T {
        let scope = Scope {
            thread_pool,
            active_counter: Arc::new(AtomicUsize::new(0)),
        };

        let result = f(&scope);

        // todo: CondVar
        while scope.active_counter.load(std::sync::atomic::Ordering::Acquire) != 0 {}

        result
    }

    pub struct Scope<'scope> {
        thread_pool: &'scope ThreadPool,
        active_counter: Arc<AtomicUsize>,
    }

    impl<'scope> Scope<'scope> {
        pub fn push_job_unhandled<F: 'scope + Send + FnOnce() -> T, T: 'scope + Send>(&'scope self, f: F) {
            _ = self.active_counter.fetch_add(1, std::sync::atomic::Ordering::AcqRel);

            let active_counter = Arc::clone(&self.active_counter);

            let f = move || {
                let exec_on_drop = ExecOnDrop::new(|| _ = active_counter.fetch_sub(1, std::sync::atomic::Ordering::AcqRel));

                f();

                drop(exec_on_drop);
            };

            let f = Box::new(f) as Box<dyn FnOnce() + Send>;

            let f = unsafe { std::mem::transmute::<Box<dyn FnOnce() + Send>, Box<dyn 'static + FnOnce() + Send>>(f) };

            self.thread_pool.push_job_unhandled(f);
        }

        pub fn panic_if_err(&self) {
            self.thread_pool.panic_if_err();
        }
    }
}

pub struct Semaphore {
    mutex: Mutex<usize>,
    cvar: Condvar,
}

impl Semaphore {
    pub fn new(initial: usize) -> Self {
        Self {
            mutex: Mutex::new(initial),
            cvar: Condvar::new(),
        }
    }

    pub fn acquire(&self) {
        let mut count = self.mutex.lock().unwrap();

        while *count == 0 {
            count = self.cvar.wait(count).unwrap();
        }

        *count -= 1;
    }

    pub fn release(&self) {
        let is_overflow = {
            let mut count = self.mutex.lock().unwrap();

            if let Some(new_count) = count.checked_add(1) {
                *count = new_count;
                false
            } else {
                true
            }
        };

        if is_overflow {
            panic!("semaphore overflow");
        }

        self.cvar.notify_one();
    }
}
