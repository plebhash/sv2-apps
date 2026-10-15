use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

// high bit of `pending_tasks`, set once `trigger_fallback_and_wait` has observed zero pending
// handlers. keeping it in the same word as the count lets the waiter observe zero and close in one
// atomic step, so a registration the waiter did not count always sees the generation closed
const CLOSED: usize = 1 << (usize::BITS - 1);

/// coordinates fallback operations across multiple components with acknowledgement.
///
/// this is meant to be used together with [`crate::task_manager::TaskManager`],
/// as it allows triggering a fallback event (via [`CancellationToken`]) and waiting
/// until all registered components have completed their cleanup.
///
/// in summary, every time we spawn a fallback-relevant task inside the manager, we MUST:
/// - call [`FallbackCoordinator::register`] at task bootstrap
/// - call [`FallbackHandler::done`] at task completion
///
/// when a fallback trigger arrives to the main status loop, we MUST call
/// [`FallbackCoordinator::trigger_fallback_and_wait`] to wait for all registered components to
/// complete their cleanup before re-initializing them under the new upstream server.
///
/// finally, a new [`FallbackCoordinator`] must be instantiated for the next fallback cycle.
///
/// # fallback generations
///
/// one coordinator instance is one fallback generation: a handler belongs to the generation of
/// the coordinator it was registered on, and a fresh coordinator starts the next generation.
///
/// registration stays legal after fallback has been triggered. a component that registers while
/// [`FallbackCoordinator::trigger_fallback_and_wait`] is still waiting joins the current
/// generation and the wait covers it. a component that registers after the wait has returned
/// belongs to a generation whose cleanup is already over: it is still counted, but the wait that
/// already returned did not cover it and only a later call on this coordinator would, so the
/// registration is logged and the component is expected to exit promptly on the already
/// cancelled token. callers avoid this case by registering before the parent task reports
/// [`FallbackHandler::done`].
#[derive(Debug, Clone)]
pub struct FallbackCoordinator {
    signal: CancellationToken,
    // pending handler count, with [`CLOSED`] in the high bit
    pending_tasks: Arc<AtomicUsize>,
    notify: Arc<Notify>,
}

impl Default for FallbackCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl FallbackCoordinator {
    pub fn new() -> Self {
        Self {
            signal: CancellationToken::new(),
            pending_tasks: Arc::new(AtomicUsize::new(0)),
            notify: Arc::new(Notify::new()),
        }
    }

    /// register a component that will participate in fallback coordination
    /// returns a [`FallbackHandler`] that must be called when the component is done
    #[must_use]
    pub fn register(&self) -> FallbackHandler {
        tracing::debug!("FallbackCoordinator: registering component");
        let prev = self.pending_tasks.fetch_add(1, Ordering::AcqRel);
        if prev & CLOSED != 0 {
            tracing::warn!(
                "FallbackCoordinator: component registered after a fallback wait on this coordinator already returned; only a later wait covers it"
            );
        }

        FallbackHandler {
            coordinator: self.clone(),
            done: AtomicBool::new(false),
        }
    }

    /// get the cancellation token that signals fallback
    pub fn token(&self) -> CancellationToken {
        self.signal.clone()
    }

    /// trigger fallback and wait until no registered component is pending
    ///
    /// the pending count is the completion condition; a wakeup only means it is worth checking
    /// again. that keeps a stale wakeup (a handler finishing and another one registering before
    /// this task resumes) from ending the wait early, and a wakeup that arrives between the
    /// check and the await is kept by [`Notify`] as a permit, so it is never missed.
    ///
    /// dropping the returned future keeps the coordinator consistent: the fallback stays
    /// triggered and a later call resumes waiting on the same pending count.
    ///
    /// one task awaits this per coordinator: the wakeup reaches a single waiter.
    pub async fn trigger_fallback_and_wait(&self) {
        tracing::debug!("FallbackCoordinator: triggering fallback");
        self.signal.cancel();

        while let Err(pending) =
            self.pending_tasks
                .compare_exchange(0, CLOSED, Ordering::AcqRel, Ordering::Acquire)
        {
            if pending == CLOSED {
                break; // closed by an earlier call
            }
            self.notify.notified().await;
        }
        tracing::debug!("FallbackCoordinator: finished waiting for components to complete cleanup");
    }
}

/// Handler for a component that will participate in fallback coordination
///
/// ⚠️ Warning: dropping this handler without calling [`FallbackHandler::done`] will result in a
/// panic.
pub struct FallbackHandler {
    coordinator: FallbackCoordinator,
    done: AtomicBool,
}

impl FallbackHandler {
    /// Mark this handler as finished
    /// Takes ownership of `self`, so calling it twice is rejected at compile time
    pub fn done(self) {
        tracing::debug!("FallbackHandler: done called");
        self.done.store(true, Ordering::Release);

        let prev = self
            .coordinator
            .pending_tasks
            .fetch_sub(1, Ordering::AcqRel);

        // every transition to zero wakes the waiter, whether or not fallback has been triggered
        // yet: `Notify` stores the wakeup as a permit while nobody is waiting, and the waiter
        // rechecks the count, so an early wakeup is harmless and a late one is never missed
        if prev & !CLOSED == 1 {
            self.coordinator.notify.notify_one();
        }
    }
}

impl Drop for FallbackHandler {
    fn drop(&mut self) {
        if !self.done.load(Ordering::Acquire) {
            panic!("FallbackHandler dropped without calling done()");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        pin::{Pin, pin},
        task::{Context, Poll, Waker},
    };

    // polls once with a no-op waker, so each test step interleaves exactly where written
    fn poll<F: Future>(fut: &mut Pin<&mut F>) -> Poll<F::Output> {
        fut.as_mut().poll(&mut Context::from_waker(Waker::noop()))
    }

    #[test]
    fn wait_returns_immediately_without_pending_handlers() {
        let coordinator = FallbackCoordinator::new();
        let handler = coordinator.register();
        handler.done();

        let mut wait = pin!(coordinator.trigger_fallback_and_wait());
        assert_eq!(poll(&mut wait), Poll::Ready(()));
        assert!(coordinator.token().is_cancelled());
    }

    #[test]
    fn wait_completes_when_the_last_handler_is_done() {
        let coordinator = FallbackCoordinator::new();
        let first = coordinator.register();
        let second = coordinator.register();

        let mut wait = pin!(coordinator.trigger_fallback_and_wait());
        assert_eq!(poll(&mut wait), Poll::Pending);

        first.done();
        assert_eq!(poll(&mut wait), Poll::Pending);

        second.done();
        assert_eq!(poll(&mut wait), Poll::Ready(()));
    }

    // Loupe #145: a handler finishing and another registering before the waiter resumes
    #[test]
    fn stale_wakeup_does_not_end_the_wait_while_a_late_handler_is_pending() {
        let coordinator = FallbackCoordinator::new();
        let early = coordinator.register();

        let mut wait = pin!(coordinator.trigger_fallback_and_wait());
        assert_eq!(poll(&mut wait), Poll::Pending);

        early.done();
        let late = coordinator.register();
        assert_eq!(poll(&mut wait), Poll::Pending);

        late.done();
        assert_eq!(poll(&mut wait), Poll::Ready(()));
    }

    // a wakeup issued before anybody waits is kept as a permit and does not count as completion
    #[test]
    fn wakeup_before_the_wait_is_rechecked_against_the_pending_count() {
        let coordinator = FallbackCoordinator::new();
        coordinator.register().done();
        let pending = coordinator.register();

        let mut wait = pin!(coordinator.trigger_fallback_and_wait());
        assert_eq!(poll(&mut wait), Poll::Pending);

        pending.done();
        assert_eq!(poll(&mut wait), Poll::Ready(()));
    }

    #[test]
    fn dropped_wait_can_be_resumed() {
        let coordinator = FallbackCoordinator::new();
        let handler = coordinator.register();

        {
            let mut wait = pin!(coordinator.trigger_fallback_and_wait());
            assert_eq!(poll(&mut wait), Poll::Pending);
        }
        assert!(coordinator.token().is_cancelled());

        let mut wait = pin!(coordinator.trigger_fallback_and_wait());
        assert_eq!(poll(&mut wait), Poll::Pending);

        handler.done();
        assert_eq!(poll(&mut wait), Poll::Ready(()));
    }

    #[test]
    fn handler_done_while_no_wait_is_polled_is_not_lost() {
        let coordinator = FallbackCoordinator::new();
        let handler = coordinator.register();

        {
            let mut wait = pin!(coordinator.trigger_fallback_and_wait());
            assert_eq!(poll(&mut wait), Poll::Pending);
        }
        handler.done();

        let mut wait = pin!(coordinator.trigger_fallback_and_wait());
        assert_eq!(poll(&mut wait), Poll::Ready(()));
    }

    #[test]
    fn registration_after_completion_is_not_covered_by_the_returned_wait() {
        let coordinator = FallbackCoordinator::new();
        let mut wait = pin!(coordinator.trigger_fallback_and_wait());
        assert_eq!(poll(&mut wait), Poll::Ready(()));

        let late = coordinator.register();
        assert_eq!(
            coordinator.pending_tasks.load(Ordering::Acquire),
            CLOSED | 1
        );
        let mut wait = pin!(coordinator.trigger_fallback_and_wait());
        assert_eq!(poll(&mut wait), Poll::Pending);
        late.done();
        assert_eq!(poll(&mut wait), Poll::Ready(()));
    }

    #[test]
    #[should_panic(expected = "FallbackHandler dropped without calling done()")]
    fn dropping_a_handler_without_done_panics() {
        let coordinator = FallbackCoordinator::new();
        let _handler = coordinator.register();
    }
}
