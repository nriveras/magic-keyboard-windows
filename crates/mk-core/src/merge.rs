//! Combines the virtual-device state of several keyboards into the reports of the one shared
//! virtual HID device, so one keyboard's releases never cancel keys another keyboard still holds.

use crate::state::{KbdReport, KeyList, Outputs, MAX_CONSUMER};

/// Keyboards tracked at once; a further one is sent unmerged.
pub const MAX_SOURCES: usize = 4;

/// Virtual-device state held by one keyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VirtualState {
    pub keyboard: KbdReport,
    pub consumer: [u16; MAX_CONSUMER],
    pub system: u8,
}

/// Reports to send to the virtual device after an update. `None` = unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Merged {
    /// Pressed state of a one-shot shortcut; send it, then `keyboard` (always set with it).
    pub pulse: Option<KbdReport>,
    pub keyboard: Option<KbdReport>,
    pub consumer: Option<[u16; MAX_CONSUMER]>,
    pub system: Option<u8>,
}

impl Merged {
    pub fn is_empty(&self) -> bool {
        self.pulse.is_none()
            && self.keyboard.is_none()
            && self.consumer.is_none()
            && self.system.is_none()
    }
}

/// Per-keyboard virtual state plus what was last sent to the device.
pub struct VirtualMerge {
    ids: [usize; MAX_SOURCES],
    states: [VirtualState; MAX_SOURCES],
    sent: VirtualState,
}

impl Default for VirtualMerge {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualMerge {
    pub const fn new() -> Self {
        const IDLE: VirtualState = VirtualState {
            keyboard: KbdReport {
                mods: 0,
                keys: KeyList::new(),
            },
            consumer: [0; MAX_CONSUMER],
            system: 0,
        };
        VirtualMerge {
            ids: [0; MAX_SOURCES],
            states: [IDLE; MAX_SOURCES],
            sent: IDLE,
        }
    }

    /// Forgets every keyboard and the sent state (the virtual device was recreated).
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Records the engine outputs of keyboard `source` (any stable non-zero ID) and returns the
    /// combined reports that changed.
    pub fn update(&mut self, source: usize, out: &Outputs) -> Merged {
        let state = VirtualState {
            keyboard: out.virt_keyboard,
            consumer: out.consumer,
            system: out.system,
        };
        let combined = match self.slot(source) {
            Some(i) => {
                self.states[i] = state;
                self.combined()
            }
            // More keyboards than slots: this one is sent as is.
            None => state,
        };

        let mut merged = Merged::default();
        if let Some(pulse) = &out.pulse {
            let mut pressed = combined.keyboard;
            add_keys(&mut pressed, pulse);
            merged.pulse = Some(pressed);
            merged.keyboard = Some(combined.keyboard);
        } else if combined.keyboard != self.sent.keyboard {
            merged.keyboard = Some(combined.keyboard);
        }
        if combined.consumer != self.sent.consumer {
            merged.consumer = Some(combined.consumer);
        }
        if combined.system != self.sent.system {
            merged.system = Some(combined.system);
        }
        self.sent = combined;
        merged
    }

    /// The slot of `source`, claiming an idle one for a new keyboard.
    fn slot(&mut self, source: usize) -> Option<usize> {
        if let Some(i) = self.ids.iter().position(|id| *id == source) {
            return Some(i);
        }
        let idle = VirtualState::default();
        let i = self.states.iter().position(|s| *s == idle)?;
        self.ids[i] = source;
        Some(i)
    }

    fn combined(&self) -> VirtualState {
        let mut all = VirtualState::default();
        let mut n_consumer = 0;
        for s in &self.states {
            add_keys(&mut all.keyboard, &s.keyboard);
            for &u in s.consumer.iter().filter(|u| **u != 0) {
                if !all.consumer.contains(&u) && n_consumer < MAX_CONSUMER {
                    all.consumer[n_consumer] = u;
                    n_consumer += 1;
                }
            }
            if all.system == 0 {
                all.system = s.system;
            }
        }
        all
    }
}

fn add_keys(into: &mut KbdReport, from: &KbdReport) {
    into.mods |= from.mods;
    for &k in from.keys.as_slice() {
        into.keys.push(k); // de-duplicates
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::usage::{consumer, kbd, modbits, system};

    const A: usize = 0x1000;
    const B: usize = 0x2000;

    fn report(mods: u8, keys: &[u8]) -> KbdReport {
        let mut list = KeyList::new();
        for &k in keys {
            list.push(k);
        }
        KbdReport { mods, keys: list }
    }

    fn consumer_out(usages: &[u16]) -> Outputs {
        let mut out = Outputs::default();
        out.consumer[..usages.len()].copy_from_slice(usages);
        out.consumer_changed = true;
        out
    }

    fn usages(c: [u16; MAX_CONSUMER]) -> Vec<u16> {
        c.iter().copied().filter(|u| *u != 0).collect()
    }

    #[test]
    fn two_keyboards_keep_each_others_media_keys() {
        let mut m = VirtualMerge::new();
        let r = m.update(A, &consumer_out(&[consumer::VOLUME_DOWN]));
        assert_eq!(usages(r.consumer.unwrap()), vec![consumer::VOLUME_DOWN]);

        let r = m.update(B, &consumer_out(&[consumer::VOLUME_UP]));
        assert_eq!(
            usages(r.consumer.unwrap()),
            vec![consumer::VOLUME_DOWN, consumer::VOLUME_UP],
            "B's press must not release A's key"
        );

        let r = m.update(A, &consumer_out(&[]));
        assert_eq!(
            usages(r.consumer.unwrap()),
            vec![consumer::VOLUME_UP],
            "A's release must not release B's key"
        );
        let r = m.update(B, &consumer_out(&[]));
        assert!(usages(r.consumer.unwrap()).is_empty());
    }

    #[test]
    fn same_usage_held_on_both_stays_down_until_both_release() {
        let mut m = VirtualMerge::new();
        m.update(A, &consumer_out(&[consumer::MUTE]));
        let r = m.update(B, &consumer_out(&[consumer::MUTE]));
        assert_eq!(r.consumer, None, "nothing new to send");
        let r = m.update(A, &consumer_out(&[]));
        assert_eq!(r.consumer, None, "still held by B");
        let r = m.update(B, &consumer_out(&[]));
        assert!(usages(r.consumer.unwrap()).is_empty());
    }

    #[test]
    fn held_virtual_keys_are_combined() {
        let mut m = VirtualMerge::new();
        let mut a = Outputs::default();
        a.virt_keyboard = report(modbits::LGUI, &[kbd::L]);
        a.virt_keyboard_changed = true;
        assert_eq!(m.update(A, &a).keyboard, Some(a.virt_keyboard));

        let mut b = Outputs::default();
        b.virt_keyboard = report(0, &[kbd::TAB]);
        b.virt_keyboard_changed = true;
        assert_eq!(
            m.update(B, &b).keyboard,
            Some(report(modbits::LGUI, &[kbd::L, kbd::TAB]))
        );

        b.virt_keyboard = KbdReport::default();
        assert_eq!(m.update(B, &b).keyboard, Some(a.virt_keyboard));
    }

    #[test]
    fn pulse_is_sent_with_the_other_keyboards_held_keys_then_released() {
        let mut m = VirtualMerge::new();
        let mut a = Outputs::default();
        a.virt_keyboard = report(modbits::LSHIFT, &[]);
        a.virt_keyboard_changed = true;
        m.update(A, &a);

        let mut b = Outputs::default();
        b.pulse = Some(report(modbits::LGUI, &[kbd::TAB]));
        let r = m.update(B, &b);
        assert_eq!(
            r.pulse,
            Some(report(modbits::LGUI | modbits::LSHIFT, &[kbd::TAB]))
        );
        assert_eq!(r.keyboard, Some(report(modbits::LSHIFT, &[])));
    }

    #[test]
    fn system_control_is_held_while_any_keyboard_holds_it() {
        let mut m = VirtualMerge::new();
        let mut a = Outputs::default();
        a.system = system::SLEEP;
        a.system_changed = true;
        assert_eq!(m.update(A, &a).system, Some(system::SLEEP));
        assert_eq!(m.update(B, &a).system, None);
        a.system = 0;
        assert_eq!(m.update(A, &a).system, None);
        assert_eq!(m.update(B, &a).system, Some(0));
    }

    #[test]
    fn unchanged_state_sends_nothing() {
        let mut m = VirtualMerge::new();
        let out = consumer_out(&[consumer::PLAY_PAUSE]);
        m.update(A, &out);
        assert!(m.update(A, &out).is_empty());
    }

    #[test]
    fn idle_slots_are_reused_by_new_keyboards() {
        let mut m = VirtualMerge::new();
        // Many keyboards come and go (each releases everything when removed).
        for id in 1..=(MAX_SOURCES * 3) {
            m.update(id, &consumer_out(&[consumer::MUTE]));
            m.update(id, &consumer_out(&[]));
        }
        m.update(A, &consumer_out(&[consumer::VOLUME_DOWN]));
        let r = m.update(B, &consumer_out(&[consumer::VOLUME_UP]));
        assert_eq!(
            usages(r.consumer.unwrap()),
            vec![consumer::VOLUME_DOWN, consumer::VOLUME_UP]
        );
    }

    #[test]
    fn reset_forgets_sent_state() {
        let mut m = VirtualMerge::new();
        let out = consumer_out(&[consumer::MUTE]);
        m.update(A, &out);
        m.reset();
        assert!(
            m.update(A, &out).consumer.is_some(),
            "resent to the new device"
        );
    }
}
