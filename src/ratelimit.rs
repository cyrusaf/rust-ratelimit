use async_channel::{Receiver, Sender};
use awaitgroup::WaitGroup;
use std::future::Future;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::sleep;

pub struct RateLimit {
    wg: WaitGroup,
    timer: Receiver<()>,
}

impl RateLimit {
    pub fn new(interval: Duration) -> Self {
        let (tx, rx) = async_channel::bounded::<()>(1);
        tokio::spawn(timer(tx, interval));

        return RateLimit {
            wg: WaitGroup::new(),
            timer: rx,
        };
    }

    pub async fn spawn<T>(&self, future: T) -> JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        let timer = self.timer.clone();
        let worker = self.wg.worker();
        // We use an async channel so that we can yield while waiting for a message
        timer.recv().await.unwrap();
        tokio::spawn(async {
            let output = future.await;
            drop(worker);
            output
        })
    }

    pub async fn wait(&mut self) {
        // We use an async waitgroup so that we can yield while waiting for the wg
        self.wg.wait().await
    }
}

async fn timer(tx: Sender<()>, interval: Duration) {
    loop {
        if let Err(_) = tx.send(()).await {
            break;
        }
        sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn spawn() {
        let (timer_tx, timer_rx) = async_channel::bounded::<()>(1);
        let rl = Arc::new(tokio::sync::Mutex::new(RateLimit {
            timer: timer_rx,
            wg: WaitGroup::new(),
        }));
        let count = Arc::new(Mutex::new(0));

        let handle = tokio::spawn({
            let rl = rl.clone();
            let count = count.clone();
            async move {
                rl.lock()
                    .await
                    .spawn(async move {
                        *count.lock().unwrap() += 1;
                    })
                    .await
            }
        });
        assert!(
            *count.lock().unwrap() == 0,
            "task executed before timer ticked"
        );

        timer_tx.send(()).await.unwrap();
        handle.await.unwrap();
        assert!(
            *count.lock().unwrap() == 1,
            "task did not execute after timer ticked"
        );

        rl.lock().await.wait().await;
    }

    #[tokio::test]
    async fn wait() {
        let (timer_tx, timer_rx) = async_channel::bounded::<()>(1);
        let rl = Arc::new(tokio::sync::Mutex::new(RateLimit {
            timer: timer_rx,
            wg: WaitGroup::new(),
        }));

        let done_waiting = Arc::new(Mutex::new(false));

        let (task_tx, task_rx) = async_channel::bounded::<()>(1);

        tokio::spawn({
            let rl = rl.clone();
            let task_rx = task_rx.clone();
            async move {
                rl.lock()
                    .await
                    .spawn(async move {
                        task_rx.recv().await.unwrap();
                    })
                    .await
            }
        });
        let wait_handle = tokio::spawn({
            let rl = rl.clone();
            let done_waiting = done_waiting.clone();
            async move {
                rl.lock().await.wait().await;
                *done_waiting.lock().unwrap() = true
            }
        });
        timer_tx.send(()).await.unwrap();
        assert!(
            *done_waiting.lock().unwrap() == false,
            "wait returned before all tasks completed"
        );

        task_tx.send(()).await.unwrap();
        wait_handle.await.unwrap();
        assert!(
            *done_waiting.lock().unwrap() == true,
            "wait returned after all tasks completed"
        );
    }
}
