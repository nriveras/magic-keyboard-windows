//! Access to the tray's per-thread state that tolerates re-entrant window messages.

use std::cell::RefCell;

/// Runs `f` on the value in `cell`. Returns `None` if the cell is empty, or if it is already
/// borrowed: a window procedure re-entered by a sent message while the state is in use (e.g.
/// during `Shell_NotifyIconW`) must skip its work rather than panic across the FFI boundary.
pub fn with_slot<T, R>(cell: &RefCell<Option<T>>, f: impl FnOnce(&mut T) -> R) -> Option<R> {
    let mut value = cell.try_borrow_mut().ok()?;
    value.as_mut().map(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_on_the_value() {
        let cell = RefCell::new(Some(1));
        assert_eq!(
            with_slot(&cell, |v| {
                *v += 1;
                *v
            }),
            Some(2)
        );
        assert_eq!(with_slot(&RefCell::new(None::<i32>), |v| *v), None);
    }

    #[test]
    fn reentrant_access_is_skipped_without_panicking() {
        let cell = RefCell::new(Some(1));
        let inner = with_slot(&cell, |_| with_slot(&cell, |v| *v));
        assert_eq!(inner, Some(None));
        assert_eq!(with_slot(&cell, |v| *v), Some(1), "usable again afterwards");
    }
}
