use std::{
    any::Any,
    collections::VecDeque,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{Arc, Condvar, Mutex, MutexGuard},
};

use super::{FfiJob, ThreadPool};

pub struct Scope<'scope, 'env: 'scope> {
    pool: Option<&'scope ThreadPool>,
    state: Arc<ScopeState>,
    _scope: PhantomData<&'scope mut &'scope ()>,
    _env: PhantomData<&'env mut &'env ()>,
}

struct ScopeState {
    jobs: Mutex<ScopeJobs>,
    cv: Condvar,
}

struct ScopeJobs {
    queue: VecDeque<FfiJob>,
    pending: usize,
    panic: Option<Box<dyn Any + Send>>,
}

impl ScopeState {
    fn lock(&self) -> MutexGuard<'_, ScopeJobs> {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn run(&self, job: FfiJob) {
        let result = catch_unwind(AssertUnwindSafe(|| job.call()));

        let mut jobs = self.lock();

        jobs.pending -= 1;

        if let Err(panic) = result {
            jobs.panic.get_or_insert(panic);
        }

        if jobs.pending == 0 {
            self.cv.notify_all();
        }
    }

    fn run_one(&self) {
        let job = self.lock().queue.pop_front();

        if let Some(job) = job {
            self.run(job);
        }
    }

    fn help_until_done(&self) -> Option<Box<dyn Any + Send>> {
        let mut jobs = self.lock();

        loop {
            if let Some(job) = jobs.queue.pop_front() {
                drop(jobs);
                self.run(job);
                jobs = self.lock();
                continue;
            }

            if jobs.pending == 0 {
                return jobs.panic.take();
            }

            jobs = self.cv.wait(jobs).unwrap_or_else(|e| e.into_inner());
        }
    }
}

impl<'scope, 'env> Scope<'scope, 'env> {
    pub fn push_job_unhandled<F: 'scope + Send + FnOnce()>(&'scope self, f: F) {
        let Some(pool) = self.pool else {
            f();
            return;
        };

        {
            let mut jobs = self.state.lock();
            // Safety. `scope` waits for every pushed job before returning, even when unwinding.
            jobs.queue.push_back(unsafe { FfiJob::new_unchecked(f) });
            jobs.pending += 1;
        }

        let state = Arc::clone(&self.state);

        pool.push_frame_job(FfiJob::new(move || state.run_one()));
    }
}

pub(super) fn scope<'env, F, R>(pool: Option<&ThreadPool>, f: F) -> R
where
    F: for<'scope> FnOnce(&'scope Scope<'scope, 'env>) -> R,
{
    let scope = Scope {
        pool,
        state: Arc::new(ScopeState {
            jobs: Mutex::new(ScopeJobs {
                queue: VecDeque::new(),
                pending: 0,
                panic: None,
            }),
            cv: Condvar::new(),
        }),
        _scope: PhantomData,
        _env: PhantomData,
    };

    let result = catch_unwind(AssertUnwindSafe(|| f(&scope)));

    let job_panic = scope.state.help_until_done();

    match (result, job_panic) {
        (Err(panic), _) | (Ok(_), Some(panic)) => resume_unwind(panic),
        (Ok(result), None) => result,
    }
}
