//! The tray's driver watcher loop: blocks on status changes and pokes the UI thread, pausing
//! between retries so a failing or instantly-returning driver call cannot spin a CPU core.

use std::io;
use std::time::Duration;

use mk_core::proto::Status;

/// Pause after a wait returned without a new status, or after the connection broke.
pub const RETRY: Duration = Duration::from_secs(1);
/// Pause between attempts to open the control interface (driver not installed or hub missing).
pub const OPEN_RETRY: Duration = Duration::from_secs(5);

/// The control-interface calls the watcher needs.
pub trait StatusSource {
    fn status(&self) -> io::Result<Status>;
    /// Blocks until the status generation differs from `generation`.
    fn wait_status(&self, generation: u32) -> io::Result<Status>;
}

/// Runs until `pause` returns false. `notify` is called whenever the UI should refresh.
pub fn run<S: StatusSource>(
    mut open: impl FnMut() -> io::Result<S>,
    mut notify: impl FnMut(),
    mut pause: impl FnMut(Duration) -> bool,
) {
    loop {
        let Ok(source) = open() else {
            if !pause(OPEN_RETRY) {
                return;
            }
            continue;
        };
        let mut generation = source.status().map(|s| s.generation).unwrap_or(0);
        notify();
        loop {
            match source.wait_status(generation) {
                Ok(status) if status.generation != generation => {
                    generation = status.generation;
                    notify();
                }
                // Returned without a change (e.g. the hub is going away): don't spin.
                Ok(_) => {
                    if !pause(RETRY) {
                        return;
                    }
                }
                Err(_) => break,
            }
        }
        // Hub removed or driver reinstalled.
        notify();
        if !pause(RETRY) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    struct Fake<'a> {
        generation: u32,
        /// Generation returned by each wait; `None` = error.
        waits: &'a Cell<Vec<Option<u32>>>,
    }

    impl StatusSource for Fake<'_> {
        fn status(&self) -> io::Result<Status> {
            Ok(Status {
                generation: self.generation,
                ..Default::default()
            })
        }

        fn wait_status(&self, _generation: u32) -> io::Result<Status> {
            let mut waits = self.waits.take();
            let next = if waits.is_empty() {
                None
            } else {
                waits.remove(0)
            };
            self.waits.set(waits);
            next.map(|generation| Status {
                generation,
                ..Default::default()
            })
            .ok_or_else(|| io::Error::other("closed"))
        }
    }

    /// Runs the loop with at most `max_pauses` pauses; returns (notifications, pauses).
    fn drive<'a>(
        mut open: impl FnMut() -> io::Result<Fake<'a>>,
        max_pauses: usize,
    ) -> (usize, Vec<Duration>) {
        let mut notes = 0;
        let mut pauses = Vec::new();
        run(
            &mut open,
            || notes += 1,
            |d| {
                pauses.push(d);
                pauses.len() < max_pauses
            },
        );
        (notes, pauses)
    }

    #[test]
    fn notifies_on_each_new_generation() {
        let waits = Cell::new(vec![Some(2), Some(3)]);
        let mut opened = 0;
        let (notes, pauses) = drive(
            || {
                opened += 1;
                Ok(Fake {
                    generation: 1,
                    waits: &waits,
                })
            },
            1,
        );
        // Connected, gen 2, gen 3, connection lost.
        assert_eq!(notes, 4);
        assert_eq!(pauses, vec![RETRY], "pauses before reconnecting");
        assert_eq!(opened, 1);
    }

    #[test]
    fn wait_that_returns_the_same_generation_pauses() {
        let waits = Cell::new(vec![Some(1); 10]);
        let (notes, pauses) = drive(
            || {
                Ok(Fake {
                    generation: 1,
                    waits: &waits,
                })
            },
            3,
        );
        assert_eq!(pauses, vec![RETRY; 3]);
        assert_eq!(notes, 1, "no refresh without a change");
    }

    #[test]
    fn failing_wait_does_not_reconnect_without_a_pause() {
        let waits = Cell::new(Vec::new());
        let mut opened = 0;
        let (_, pauses) = drive(
            || {
                opened += 1;
                Ok(Fake {
                    generation: 1,
                    waits: &waits,
                })
            },
            5,
        );
        assert_eq!(opened, 5, "one reconnect per pause");
        assert_eq!(pauses, vec![RETRY; 5]);
    }

    #[test]
    fn open_failure_waits_longer() {
        let (notes, pauses) = drive(|| Err(io::Error::other("no driver")), 2);
        assert_eq!(notes, 0);
        assert_eq!(pauses, vec![OPEN_RETRY; 2]);
    }
}
