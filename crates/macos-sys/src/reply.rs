//! Hands a value made on another thread (a system completion handler, a worker) to a callback that
//! waits on the main thread, where the interface lives.

use crate::app;
use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::HashMap,
    marker::PhantomData,
};

type Waiting = Box<dyn FnOnce(Box<dyn Any>)>;

thread_local! {
    static WAITING: RefCell<HashMap<u64, Waiting>> = RefCell::new(HashMap::new());
    static NEXT: Cell<u64> = const { Cell::new(1) };
}

/// Sendable to any thread and redeemable once there for the callback registered by [`expect`].
pub struct Ticket<T> {
    id: u64,
    sent: bool,
    _value: PhantomData<fn(T)>,
}

/// Registers `done`; call on the main thread.
pub fn expect<T: Send + 'static>(done: impl FnOnce(T) + 'static) -> Ticket<T> {
    let id = NEXT.with(|next| {
        let id = next.get();
        next.set(id + 1);
        id
    });
    let waiting: Waiting = Box::new(move |value: Box<dyn Any>| {
        if let Ok(value) = value.downcast::<T>() {
            done(*value);
        }
    });
    WAITING.with(|w| w.borrow_mut().insert(id, waiting));
    Ticket {
        id,
        sent: false,
        _value: PhantomData,
    }
}

impl<T: Send + 'static> Ticket<T> {
    /// Runs the waiting callback with `value` on the main thread, soon.
    pub fn send(mut self, value: T) {
        self.sent = true;
        let id = self.id;
        app::on_main(move || {
            let done = WAITING.with(|w| w.borrow_mut().remove(&id));
            if let Some(done) = done {
                done(Box::new(value));
            }
        });
    }
}

impl<T> Drop for Ticket<T> {
    fn drop(&mut self) {
        if !self.sent {
            let id = self.id;
            app::on_main(move || {
                WAITING.with(|w| w.borrow_mut().remove(&id));
            });
        }
    }
}
