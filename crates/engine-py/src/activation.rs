//! T-051: `Window.request_activation_token(callback, app_id=None)`.
//!
//! A launcher asks the compositor for an `xdg_activation_v1` token and starts the app with
//! `XDG_ACTIVATION_TOKEN` set to it, so the compositor lets the new window take focus. The
//! answer is not immediate: the window keeps the callbacks here until it comes.
//!
//! A layer surface's request carries our own number through the layer thread and back; an
//! ordinary window's goes through `winit`, whose answer names its own serial, so those
//! callbacks are answered in the order they were asked (one connection, one queue).

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use pyo3::prelude::*;

use crate::dispatch::log_uncaught_exception;

#[derive(Default)]
pub(crate) struct ActivationRequests {
    next: u64,
    callbacks: HashMap<u64, Py<PyAny>>,
    /// The requests made through `winit`, oldest first.
    winit_order: VecDeque<u64>,
}

pub(crate) type SharedActivation = Rc<RefCell<ActivationRequests>>;

impl ActivationRequests {
    pub(crate) fn add(&mut self, callback: Py<PyAny>) -> u64 {
        self.next += 1;
        self.callbacks.insert(self.next, callback);
        self.next
    }

    /// Takes a request back (it could not be made).
    pub(crate) fn forget(&mut self, id: u64) -> Option<Py<PyAny>> {
        self.winit_order.retain(|&queued| queued != id);
        self.callbacks.remove(&id)
    }

    pub(crate) fn queue_winit(&mut self, id: u64) {
        self.winit_order.push_back(id);
    }

    pub(crate) fn callbacks(&self) -> impl Iterator<Item = &Py<PyAny>> {
        self.callbacks.values()
    }

    pub(crate) fn clear(&mut self) {
        self.callbacks.clear();
        self.winit_order.clear();
    }
}

/// Calls the callback of request `id` (if it is still waiting) with the token. An exception
/// in it is logged.
pub(crate) fn deliver(requests: &SharedActivation, id: u64, token: Option<String>, py: Python<'_>) {
    let callback = requests.borrow_mut().forget(id);
    if let Some(callback) = callback
        && let Err(err) = callback.call1(py, (token,))
    {
        log_uncaught_exception(&err, py);
    }
}

/// `winit` answered: the oldest request made through it gets the token.
pub(crate) fn deliver_winit(requests: &SharedActivation, token: String, py: Python<'_>) {
    let id = requests.borrow_mut().winit_order.pop_front();
    if let Some(id) = id {
        deliver(requests, id, Some(token), py);
    }
}
