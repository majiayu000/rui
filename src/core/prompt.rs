//! File dialogs: asking the user for paths to open or a path to save to.
//!
//! [`AppContext::prompt_for_paths`] and [`AppContext::prompt_for_new_path`]
//! queue a [`PathPrompt`] for the platform and return a [`Task`] that finishes
//! with the answer. The macOS runner shows `NSOpenPanel` / `NSSavePanel`;
//! headless tests take the prompt and answer it themselves.
//!
//! [`AppContext::prompt_for_paths`]: crate::core::AppContext::prompt_for_paths
//! [`AppContext::prompt_for_new_path`]: crate::core::AppContext::prompt_for_new_path
//! [`Task`]: crate::core::Task

use std::cell::RefCell;
use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

/// What an open prompt lets the user choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathPromptOptions {
    pub files: bool,
    pub directories: bool,
    pub multiple: bool,
}

impl Default for PathPromptOptions {
    /// One file.
    fn default() -> Self {
        Self {
            files: true,
            directories: false,
            multiple: false,
        }
    }
}

/// A prompt waiting for the platform to show it.
#[derive(Debug)]
pub enum PathPrompt {
    /// Choose existing paths. The answer is `None` when the user cancels.
    Open {
        options: PathPromptOptions,
        answer: PromptAnswer<Option<Vec<PathBuf>>>,
    },
    /// Choose a path to save to, starting in `directory`. The answer is `None`
    /// when the user cancels.
    Save {
        directory: PathBuf,
        answer: PromptAnswer<Option<PathBuf>>,
    },
}

/// The prompt ended without an answer: whoever showed it dropped it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptDismissed;

impl fmt::Display for PromptDismissed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the path prompt was dropped without an answer")
    }
}

impl std::error::Error for PromptDismissed {}

struct Slot<T> {
    value: Option<Result<T, PromptDismissed>>,
    waiter: Option<Waker>,
}

impl<T> Slot<T> {
    fn complete(&mut self, value: Result<T, PromptDismissed>) {
        if self.value.is_none() {
            self.value = Some(value);
        }
        if let Some(waiter) = self.waiter.take() {
            waiter.wake();
        }
    }
}

/// Where a prompt's answer goes. Dropping it unanswered fails the waiting
/// task with [`PromptDismissed`].
pub struct PromptAnswer<T> {
    slot: Rc<RefCell<Slot<T>>>,
    answered: bool,
}

impl<T> PromptAnswer<T> {
    pub fn send(mut self, value: T) {
        self.answered = true;
        self.slot.borrow_mut().complete(Ok(value));
    }
}

impl<T> Drop for PromptAnswer<T> {
    fn drop(&mut self) {
        if !self.answered {
            self.slot.borrow_mut().complete(Err(PromptDismissed));
        }
    }
}

impl<T> fmt::Debug for PromptAnswer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PromptAnswer").finish_non_exhaustive()
    }
}

/// Resolves when the paired [`PromptAnswer`] is sent or dropped.
pub(crate) struct PromptReply<T> {
    slot: Rc<RefCell<Slot<T>>>,
}

impl<T> Future for PromptReply<T> {
    type Output = Result<T, PromptDismissed>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut slot = self.slot.borrow_mut();
        match slot.value.take() {
            Some(value) => Poll::Ready(value),
            None => {
                slot.waiter = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

pub(crate) fn prompt_reply<T>() -> (PromptAnswer<T>, PromptReply<T>) {
    let slot = Rc::new(RefCell::new(Slot {
        value: None,
        waiter: None,
    }));
    (
        PromptAnswer {
            slot: Rc::clone(&slot),
            answered: false,
        },
        PromptReply { slot },
    )
}
