#![allow(clippy::field_reassign_with_default)]

//! Property tests: no stuck keys, bounded outputs, for random configs and key sequences.

use mk_core::config::{
    ClearAction, Config, EjectAction, F6Action, FnDoubleTap, FnMode, LockAction, ModSource, ModTarget,
};
use mk_core::state::{KbdReport, MAX_KEYS};
use mk_core::usage::kbd;
use mk_core::{Engine, KeyState, Outputs};
use proptest::prelude::*;

/// Keys that exercise every engine path.
const UNIVERSE: &[u8] = &[
    kbd::A,
    kbd::P,
    kbd::SPACE,
    kbd::BACKSPACE,
    kbd::ENTER,
    kbd::ESCAPE,
    kbd::LEFT,
    kbd::UP,
    kbd::CAPS_LOCK,
    kbd::NUM_LOCK_CLEAR,
    0x3A, // F1
    0x3C, // F3
    0x3F, // F6
    0x41, // F8
    kbd::F11,
    kbd::F12,
    kbd::F13,
    kbd::F16,
];

fn pick<T: Copy>(all: &[T], i: u8) -> T {
    all[i as usize % all.len()]
}

fn arb_config() -> impl Strategy<Value = Config> {
    (any::<[u8; 7]>(), any::<[u8; ModSource::COUNT]>(), any::<bool>()).prop_map(|(e, mods, sleep)| {
        let mut cfg = Config::default();
        cfg.fn_mode = pick(FnMode::ALL, e[0]);
        cfg.f6 = pick(F6Action::ALL, e[1]);
        cfg.clear = pick(ClearAction::ALL, e[2]);
        cfg.lock = pick(LockAction::ALL, e[3]);
        cfg.eject = pick(EjectAction::ALL, e[4]);
        cfg.fn_double_tap = pick(FnDoubleTap::ALL, e[5]);
        cfg.sleep_combo = sleep;
        for (i, m) in mods.iter().enumerate() {
            cfg.modifiers[i] = pick(ModTarget::ALL, *m);
        }
        if e[6] & 1 == 1 {
            cfg.f_ext[0] = 0;
        }
        cfg
    })
}

#[derive(Clone, Debug)]
struct Frame {
    state: KeyState,
    dt: u32,
}

fn arb_frame() -> impl Strategy<Value = Frame> {
    (any::<u8>(), prop::collection::vec(0..UNIVERSE.len(), 0..5), any::<[bool; 4]>(), 1u32..600).prop_map(
        |(mods, keys, specials, dt)| {
            let mut s = KeyState::new();
            s.mods = mods;
            for k in keys {
                s.press(UNIVERSE[k]);
            }
            s.fn_key = specials[0];
            s.eject = specials[1];
            s.menu = specials[2];
            s.lock = specials[3];
            Frame { state: s, dt }
        },
    )
}

fn assert_bounded(out: &Outputs) {
    assert!(out.keyboard.keys.len() <= MAX_KEYS);
    assert!(out.consumer_usages().count() <= mk_core::state::MAX_CONSUMER);
}

fn assert_idle(out: &Outputs) {
    assert_eq!(out.keyboard, KbdReport::default(), "keyboard report must be empty");
    assert_eq!(out.virt_keyboard, KbdReport::default(), "virtual keyboard must be released");
    assert_eq!(out.consumer_usages().count(), 0, "consumer must be released");
    assert_eq!(out.system, 0, "system control must be released");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn no_stuck_keys(cfg in arb_config(), frames in prop::collection::vec(arb_frame(), 1..40)) {
        let mut e = Engine::new(cfg);
        let mut out = Outputs::default();
        let mut t = 0u32;
        for f in &frames {
            t = t.wrapping_add(f.dt);
            e.process(&f.state, t, &mut out);
            assert_bounded(&out);
        }
        t = t.wrapping_add(5);
        e.process(&KeyState::new(), t, &mut out);
        assert_idle(&out);
        // Timers firing afterwards must not leave anything held either.
        if let Some(d) = e.next_deadline() {
            e.tick(d, &mut out);
            assert_idle(&out);
        }
    }

    #[test]
    fn idle_reports_are_stable(cfg in arb_config(), frames in prop::collection::vec(arb_frame(), 1..20)) {
        // Repeating the same report must not produce virtual-device changes the second time,
        // unless a timer fired in between (time is held constant here so none can).
        let mut e = Engine::new(cfg);
        let mut out = Outputs::default();
        let mut t = 0u32;
        for f in &frames {
            t = t.wrapping_add(f.dt);
            e.process(&f.state, t, &mut out);
            e.process(&f.state, t, &mut out);
            prop_assert!(!out.virt_keyboard_changed && !out.consumer_changed && !out.system_changed);
        }
    }
}
