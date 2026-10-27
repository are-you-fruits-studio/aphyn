use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

pub fn block_on<F: IntoFuture>(fut: F) -> F::Output {
    let mut fut = std::pin::pin!(fut.into_future());
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);

    loop {
        match fut.as_mut().poll(&mut context) {
            Poll::Pending => {
                println!("poll pending");
            },
            Poll::Ready(result) => {
                println!("poll ready");
                return result;
            },
        }
    }
}

struct YieldNow {
    did_yield: bool,
}

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.did_yield {
            Poll::Ready(())
        } else {
            self.did_yield = true;           
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

pub fn yield_now() -> impl Future {
    YieldNow { did_yield: false }
}

// example
async fn example_future_func() -> i32 {
    for i in 0..10 {
        yield_now().await;
    }

    123
}