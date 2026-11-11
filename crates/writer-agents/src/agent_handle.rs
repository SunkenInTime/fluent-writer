//! Runs one request on a worker thread and hands its events to the UI. This stands in for the
//! main-queue callbacks the Swift sessions used: every session call happens on the worker, events
//! go over a channel, and a caller-supplied wake callback runs after each one so the UI can poll.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::agent_models::{AgentEvent, AgentPrompt, AgentRequest, AgentSession, ProcessSink};
use crate::claude_agent::AgentFactory;
use crate::line_process::ProcessEvent;

enum Control {
    Process(ProcessEvent),
    Stop,
}

/// One running request. Poll it with [`AgentHandle::try_recv`] after the wake callback fires.
/// Dropping the handle stops the request.
pub struct AgentHandle {
    events: Mutex<Receiver<AgentEvent>>,
    control: Sender<Control>,
    stopped: Arc<AtomicBool>,
}

impl AgentHandle {
    /// Builds the prompt for `request`, picks the agent with [`AgentFactory::make`] (honouring
    /// `FLUENT_WRITER_AGENT_FIXTURE`) and starts it.
    pub fn start_request(request: &AgentRequest, wake: impl Fn() + Send + 'static) -> AgentHandle {
        Self::start(
            AgentFactory::make(request.provider),
            AgentPrompt::build(request),
            wake,
        )
    }

    /// Starts `session` with `prompt` on a worker thread. `wake` runs on a background thread after
    /// each event is queued; it should only nudge the UI (post a redraw), not block.
    pub fn start(
        session: Box<dyn AgentSession>,
        prompt: String,
        wake: impl Fn() + Send + 'static,
    ) -> AgentHandle {
        let (event_tx, event_rx) = mpsc::channel::<AgentEvent>();
        let (control_tx, control_rx) = mpsc::channel::<Control>();
        let stopped = Arc::new(AtomicBool::new(false));
        let handle = AgentHandle {
            events: Mutex::new(event_rx),
            control: control_tx.clone(),
            stopped: stopped.clone(),
        };
        let sink: ProcessSink = Arc::new(move |e| {
            let _ = control_tx.send(Control::Process(e));
        });
        let flag = stopped;
        let on_event = Box::new(move |e: AgentEvent| {
            if flag.load(Ordering::SeqCst) {
                return;
            }
            if event_tx.send(e).is_ok() {
                wake();
            }
        });
        thread::Builder::new()
            .name("fluent-writer-agent".into())
            .spawn(move || run(session, prompt, on_event, sink, control_rx))
            .expect("spawn agent thread");
        handle
    }

    /// The next event, if one is waiting. Always `None` after [`AgentHandle::stop`].
    pub fn try_recv(&self) -> Option<AgentEvent> {
        if self.is_stopped() {
            return None;
        }
        self.events.lock().ok()?.try_recv().ok()
    }

    /// Waits up to `timeout` for the next event. `None` on timeout, after the request ended and
    /// its events were taken, or after [`AgentHandle::stop`].
    pub fn recv_timeout(&self, timeout: Duration) -> Option<AgentEvent> {
        if self.is_stopped() {
            return None;
        }
        let e = self.events.lock().ok()?.recv_timeout(timeout).ok()?;
        (!self.is_stopped()).then_some(e)
    }

    /// Stops the request and kills its child process. No events are delivered after this returns.
    pub fn stop(&self) {
        if !self.stopped.swap(true, Ordering::SeqCst) {
            let _ = self.control.send(Control::Stop);
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }
}

impl Drop for AgentHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

impl std::fmt::Debug for AgentHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHandle")
            .field("stopped", &self.is_stopped())
            .finish()
    }
}

fn run(
    mut session: Box<dyn AgentSession>,
    prompt: String,
    on_event: crate::agent_models::EventHandler,
    sink: ProcessSink,
    control: Receiver<Control>,
) {
    session.start(&prompt, on_event, sink);
    while !session.is_finished() {
        match control.recv_timeout(Duration::from_millis(100)) {
            Ok(Control::Process(e)) => session.handle(e),
            Ok(Control::Stop) | Err(RecvTimeoutError::Disconnected) => {
                session.cancel();
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}
