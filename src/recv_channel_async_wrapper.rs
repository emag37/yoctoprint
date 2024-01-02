use std::{task::Waker, sync::{Arc, Mutex}, time::Duration};

use crossbeam::channel::Receiver;
use log::error;
use rocket::futures::Stream;
use noop_waker::noop_waker; // I will be included in Task::Waker eventually...

struct SharedState<T> {
    next_value: Option<T>,
    next_waker: Waker,
    run: bool
}

pub struct RecvChannelAsyncWrapper<T> {
    shared_state: Arc<Mutex<SharedState<T>>>,
    reader_thread: Option<std::thread::JoinHandle<()>>
}

impl<T> RecvChannelAsyncWrapper<T> where T: Send + 'static {
    fn poll_channel(shared_state: Arc<Mutex<SharedState<T>>>, receiver: Receiver<T>) {
        loop {
            let next_value_empty;
            let mut next_waker = noop_waker();

            {
                let locked = shared_state.lock().unwrap();
                next_value_empty = locked.next_value.is_none();

                if !locked.run {
                    break;
                }
            }

            if next_value_empty {
                match receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(msg) => {
                        let mut locked = shared_state.lock().unwrap();
                        locked.next_value = Some(msg);
                        next_waker = locked.next_waker.clone();
                    },
                    Err(e) => {
                        if e.is_disconnected() {
                            error!("Error receiving from console: {:?}", e);
                            break;
                        } else {
                            continue;
                        }
                    }
                }
            }

            next_waker.wake();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    pub fn new(receiver: Receiver<T>) -> Self {
        let shared_state = Arc::new(Mutex::new(SharedState { next_value: None, next_waker: noop_waker(), run: true }));
        let shared_state_for_thread: Arc<Mutex<SharedState<T>>> = shared_state.clone();

        let reader_fn = move || {
            Self::poll_channel(shared_state_for_thread, receiver)
        };
        let ret_wrapper = RecvChannelAsyncWrapper {
            shared_state: shared_state.clone(),
            reader_thread: Some(std::thread::spawn(reader_fn))
        };
        return ret_wrapper;
    }

}

impl<T> Drop for RecvChannelAsyncWrapper<T> {
    fn drop(&mut self) {
        self.shared_state.lock().unwrap().run = false;
        let to_join = self.reader_thread.take().unwrap();
        let _ = to_join.join();
    }
}

impl<T> Unpin for RecvChannelAsyncWrapper<T> {

}

impl<T> Stream for RecvChannelAsyncWrapper<T> {
    type Item = T;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>)
        -> std::task::Poll<Option<Self::Item>> {
            let next_value = self.shared_state.lock().unwrap().next_value.take();

            if next_value.is_some() {
                std::task::Poll::Ready(next_value)
            } else if self.reader_thread.as_ref().unwrap().is_finished() {
                std::task::Poll::Ready(None)
            } else {
                self.shared_state.lock().unwrap().next_waker = cx.waker().clone();
                std::task::Poll::Pending
            }
        }
}