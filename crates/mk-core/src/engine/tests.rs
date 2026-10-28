#![allow(clippy::field_reassign_with_default)]

use super::*;
use crate::config::{ModPreset, F_EXT_COUNT};
use crate::state::KeyState;

/// Test harness: feeds reports 10 ms apart unless a time is given.
struct H {
    e: Engine,
    t: u32,
}

impl H {
    fn new() -> Self {
        Self::with(Config::default())
    }

    fn with(cfg: Config) -> Self {
        H { e: Engine::new(cfg), t: 1000 }
    }

    fn send(&mut self, s: KeyState) -> Outputs {
        self.t += 10;
        self.send_at(self.t, s)
    }

    fn send_at(&mut self, t: u32, s: KeyState) -> Outputs {
        self.t = t;
        let mut out = Outputs::default();
        self.e.process(&s, t, &mut out);
        out
    }

    fn tick_at(&mut self, t: u32) -> Outputs {
        self.t = t;
        let mut out = Outputs::default();
        self.e.tick(t, &mut out);
        out
    }
}

fn ks(keys: &[u8]) -> KeyState {
    let mut s = KeyState::new();
    for &k in keys {
        s.press(k);
    }
    s
}

fn fnks(keys: &[u8]) -> KeyState {
    ks(keys).with_fn(true)
}

fn none() -> KeyState {
    KeyState::new()
}

fn keys(out: &Outputs) -> &[u8] {
    out.keyboard.keys.as_slice()
}

fn consumer(out: &Outputs) -> Vec<u16> {
    out.consumer_usages().collect()
}

fn virt(mods: u8, key: u8) -> KbdReport {
    let mut keys = KeyList::new();
    keys.push(key);
    KbdReport { mods, keys }
}

// ---- basics ---------------------------------------------------------------------------------

#[test]
fn letters_pass_through() {
    let mut h = H::new();
    let out = h.send(ks(&[kbd::A, kbd::LEFT_SHIFT]));
    assert_eq!(keys(&out), &[kbd::A]);
    assert_eq!(out.keyboard.mods, modbits::LSHIFT);
    assert!(!out.any_virtual_change());
    let out = h.send(none());
    assert!(out.keyboard.keys.is_empty());
    assert_eq!(out.keyboard.mods, 0);
}

#[test]
fn fn_alone_produces_nothing() {
    let mut h = H::new();
    let out = h.send(fnks(&[]));
    assert_eq!(out.keyboard, KbdReport::default());
    assert!(!out.any_virtual_change());
}

// ---- function row ---------------------------------------------------------------------------

#[test]
fn media_first_top_row_is_media() {
    let mut h = H::new();
    let out = h.send(ks(&[0x41])); // F8
    assert!(keys(&out).is_empty());
    assert_eq!(consumer(&out), vec![consumer::PLAY_PAUSE]);
    assert!(out.consumer_changed);
    let out = h.send(ks(&[0x41]));
    assert!(!out.consumer_changed, "held key must not resubmit");
    let out = h.send(none());
    assert!(consumer(&out).is_empty());
    assert!(out.consumer_changed);
}

#[test]
fn all_special_functions() {
    let cases: [(u8, Action); 12] = [
        (0x3A, Action::Consumer(consumer::BRIGHTNESS_DOWN)),
        (0x3B, Action::Consumer(consumer::BRIGHTNESS_UP)),
        (0x3C, Action::Combo { mods: modbits::LGUI, key: kbd::TAB }),
        (0x3D, Action::Combo { mods: modbits::LGUI, key: kbd::S }),
        (0x3E, Action::Combo { mods: modbits::LGUI, key: kbd::H }),
        (0x3F, Action::System(system::SLEEP)),
        (0x40, Action::Consumer(consumer::PREV_TRACK)),
        (0x41, Action::Consumer(consumer::PLAY_PAUSE)),
        (0x42, Action::Consumer(consumer::NEXT_TRACK)),
        (0x43, Action::Consumer(consumer::MUTE)),
        (0x44, Action::Consumer(consumer::VOLUME_DOWN)),
        (0x45, Action::Consumer(consumer::VOLUME_UP)),
    ];
    let e = Engine::new(Config::default());
    for (k, expected) in cases {
        assert_eq!(e.resolve(k as PhysKey), expected, "key {k:#x}");
    }
}

#[test]
fn fn_gives_f_keys_in_media_first_mode() {
    let mut h = H::new();
    let out = h.send(fnks(&[0x41]));
    assert_eq!(keys(&out), &[0x41]);
    assert!(consumer(&out).is_empty());
}

#[test]
fn f_keys_first_mode_inverts() {
    let mut cfg = Config::default();
    cfg.fn_mode = FnMode::FKeysFirst;
    let mut h = H::with(cfg);
    assert_eq!(keys(&h.send(ks(&[0x45]))), &[0x45]);
    h.send(none());
    let out = h.send(fnks(&[0x45]));
    assert_eq!(consumer(&out), vec![consumer::VOLUME_UP]);
}

#[test]
fn shift_media_variants() {
    let mut h = H::new();
    assert_eq!(consumer(&h.send(ks(&[kbd::LEFT_SHIFT, 0x40]))), vec![consumer::REWIND]);
    h.send(none());
    assert_eq!(consumer(&h.send(ks(&[kbd::RIGHT_SHIFT, 0x41]))), vec![consumer::STOP]);
    h.send(none());
    assert_eq!(consumer(&h.send(ks(&[kbd::LEFT_SHIFT, 0x42]))), vec![consumer::FAST_FORWARD]);
}

#[test]
fn win_shortcuts_go_to_virtual_keyboard() {
    let mut h = H::new();
    let out = h.send(ks(&[0x3C])); // F3
    assert_eq!(out.virt_keyboard, virt(modbits::LGUI, kbd::TAB));
    assert!(out.virt_keyboard_changed);
    assert!(keys(&out).is_empty());
    let out = h.send(none());
    assert_eq!(out.virt_keyboard, KbdReport::default());
    assert!(out.virt_keyboard_changed);
}

#[test]
fn f6_options() {
    let mut cfg = Config::default();
    let mut h = H::with(cfg);
    let out = h.send(ks(&[0x3F]));
    assert_eq!(out.system, system::SLEEP);
    assert!(out.system_changed);

    cfg.f6 = F6Action::DoNotDisturb;
    let mut h = H::with(cfg);
    assert_eq!(h.send(ks(&[0x3F])).virt_keyboard, virt(modbits::LGUI, kbd::N));

    cfg.f6 = F6Action::None;
    let mut h = H::with(cfg);
    let out = h.send(ks(&[0x3F]));
    assert!(keys(&out).is_empty() && !out.any_virtual_change());
}

// ---- latching -------------------------------------------------------------------------------

#[test]
fn releasing_fn_while_key_held_keeps_latched_output() {
    let mut h = H::new();
    assert_eq!(keys(&h.send(fnks(&[0x41]))), &[0x41]);
    // Fn released, F8 still held: must stay F8, not switch to Play/Pause.
    let out = h.send(ks(&[0x41]));
    assert_eq!(keys(&out), &[0x41]);
    assert!(consumer(&out).is_empty());
    assert!(keys(&h.send(none())).is_empty());
}

#[test]
fn pressing_fn_while_media_key_held_keeps_media() {
    let mut h = H::new();
    assert_eq!(consumer(&h.send(ks(&[0x45]))), vec![consumer::VOLUME_UP]);
    let out = h.send(fnks(&[0x45]));
    assert_eq!(consumer(&out), vec![consumer::VOLUME_UP]);
    assert!(keys(&out).is_empty());
    let out = h.send(fnks(&[]));
    assert!(consumer(&out).is_empty());
}

#[test]
fn fn_and_key_in_same_report_combine() {
    let mut h = H::new();
    assert_eq!(keys(&h.send(fnks(&[kbd::BACKSPACE]))), &[kbd::DELETE]);
}

#[test]
fn shift_released_before_media_key() {
    let mut h = H::new();
    assert_eq!(consumer(&h.send(ks(&[kbd::LEFT_SHIFT, 0x41]))), vec![consumer::STOP]);
    assert_eq!(consumer(&h.send(ks(&[0x41]))), vec![consumer::STOP]);
    assert!(consumer(&h.send(none())).is_empty());
}

// ---- missing keys ---------------------------------------------------------------------------

#[test]
fn fn_navigation_keys() {
    let cases = [
        (kbd::BACKSPACE, kbd::DELETE),
        (kbd::ENTER, kbd::INSERT),
        (kbd::LEFT, kbd::HOME),
        (kbd::RIGHT, kbd::END),
        (kbd::UP, kbd::PAGE_UP),
        (kbd::DOWN, kbd::PAGE_DOWN),
        (kbd::ESCAPE, kbd::PAUSE),
        (kbd::P, kbd::PRINT_SCREEN),
    ];
    for (from, to) in cases {
        let mut h = H::new();
        assert_eq!(keys(&h.send(fnks(&[from]))), &[to], "fn+{from:#x}");
        assert!(keys(&h.send(none())).is_empty());
    }
}

#[test]
fn fn_p_keeps_other_modifiers() {
    let mut h = H::new();
    let out = h.send(fnks(&[kbd::LEFT_ALT, kbd::P]));
    assert_eq!(keys(&out), &[kbd::PRINT_SCREEN]);
    assert_eq!(out.keyboard.mods, modbits::LALT);
}

#[test]
fn fn_left_shift_f11_is_print_screen_without_shift() {
    let mut h = H::new();
    let out = h.send(fnks(&[kbd::LEFT_SHIFT, kbd::LEFT_ALT, kbd::F11]));
    assert_eq!(keys(&out), &[kbd::PRINT_SCREEN]);
    assert_eq!(out.keyboard.mods, modbits::LALT, "left shift is consumed, alt passes through");
    let out = h.send(fnks(&[kbd::LEFT_SHIFT, kbd::LEFT_ALT]));
    assert_eq!(out.keyboard.mods, modbits::LSHIFT | modbits::LALT, "shift returns after release");
}

#[test]
fn fn_left_shift_f12_is_scroll_lock() {
    let mut h = H::new();
    let out = h.send(fnks(&[kbd::LEFT_SHIFT, kbd::F12]));
    assert_eq!(keys(&out), &[kbd::SCROLL_LOCK]);
    assert_eq!(out.keyboard.mods, 0);
}

#[test]
fn ctrl_alt_delete() {
    let mut h = H::new();
    let out = h.send(fnks(&[kbd::LEFT_CTRL, kbd::LEFT_ALT, kbd::BACKSPACE]));
    assert_eq!(keys(&out), &[kbd::DELETE]);
    assert_eq!(out.keyboard.mods, modbits::LCTRL | modbits::LALT);
}

// ---- extended F-keys ------------------------------------------------------------------------

#[test]
fn f13_to_f19_defaults() {
    let expected = [kbd::PRINT_SCREEN, kbd::SCROLL_LOCK, kbd::PAUSE, kbd::F16, kbd::F17, kbd::F18, kbd::F19];
    for i in 0..F_EXT_COUNT as u8 {
        let mut h = H::new();
        assert_eq!(keys(&h.send(ks(&[kbd::F13 + i]))), &[expected[i as usize]]);
    }
}

#[test]
fn f_ext_disabled() {
    let mut cfg = Config::default();
    cfg.f_ext[0] = 0;
    let mut h = H::with(cfg);
    assert!(keys(&h.send(ks(&[kbd::F13]))).is_empty());
}

// ---- modifiers ------------------------------------------------------------------------------

#[test]
fn default_modifiers_are_identity() {
    let mut h = H::new();
    let all = [0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7];
    assert_eq!(h.send(ks(&all)).keyboard.mods, 0xFF);
}

#[test]
fn pc_order_preset_swaps_option_and_command() {
    let mut cfg = Config::default();
    cfg.apply_preset(ModPreset::PcOrder);
    let mut h = H::with(cfg);
    assert_eq!(h.send(ks(&[kbd::LEFT_GUI])).keyboard.mods, modbits::LALT);
    h.send(none());
    assert_eq!(h.send(ks(&[kbd::RIGHT_ALT])).keyboard.mods, modbits::RGUI);
}

#[test]
fn mac_like_preset() {
    let mut cfg = Config::default();
    cfg.apply_preset(ModPreset::MacLikeShortcuts);
    let mut h = H::with(cfg);
    let out = h.send(ks(&[kbd::LEFT_GUI, 0x06])); // ⌘C
    assert_eq!(out.keyboard.mods, modbits::LCTRL);
    assert_eq!(keys(&out), &[0x06]);
}

#[test]
fn right_command_as_altgr() {
    let mut cfg = Config::default();
    cfg.set_modifier(ModSource::RightCommand, ModTarget::AltGr);
    let mut h = H::with(cfg);
    assert_eq!(h.send(ks(&[kbd::RIGHT_GUI])).keyboard.mods, modbits::RALT);
}

#[test]
fn caps_lock_as_fn_and_fn_as_ctrl() {
    let mut cfg = Config::default();
    cfg.set_modifier(ModSource::CapsLock, ModTarget::Fn);
    cfg.set_modifier(ModSource::Fn, ModTarget::Ctrl);
    let mut h = H::with(cfg);
    assert_eq!(keys(&h.send(ks(&[kbd::CAPS_LOCK, kbd::BACKSPACE]))), &[kbd::DELETE]);
    h.send(none());
    let out = h.send(fnks(&[0x06]));
    assert_eq!(out.keyboard.mods, modbits::LCTRL);
    assert_eq!(keys(&out), &[0x06]);
}

#[test]
fn disabled_and_non_modifier_targets() {
    let mut cfg = Config::default();
    cfg.set_modifier(ModSource::CapsLock, ModTarget::Disabled);
    cfg.set_modifier(ModSource::RightOption, ModTarget::Menu);
    cfg.set_modifier(ModSource::RightControl, ModTarget::Insert);
    let mut h = H::with(cfg);
    let out = h.send(ks(&[kbd::CAPS_LOCK, kbd::RIGHT_ALT, kbd::RIGHT_CTRL]));
    assert_eq!(out.keyboard.mods, 0);
    // Modifier bits are processed in bit order: RCtrl (0xE4) before RAlt (0xE6).
    assert_eq!(keys(&out), &[kbd::INSERT, kbd::APPLICATION]);
}

// ---- other keys -----------------------------------------------------------------------------

#[test]
fn clear_key_options() {
    for (action, expected) in [
        (ClearAction::NumLock, Some(kbd::NUM_LOCK_CLEAR)),
        (ClearAction::Delete, Some(kbd::DELETE)),
        (ClearAction::Backspace, Some(kbd::BACKSPACE)),
        (ClearAction::Disabled, None),
    ] {
        let mut cfg = Config::default();
        cfg.clear = action;
        let mut h = H::with(cfg);
        let out = h.send(ks(&[kbd::NUM_LOCK_CLEAR]));
        assert_eq!(keys(&out), expected.as_slice(), "{action:?}");
    }
}

#[test]
fn lock_key_locks_workstation() {
    let mut h = H::new();
    let mut s = none();
    s.lock = true;
    let out = h.send(s);
    assert_eq!(out.virt_keyboard, virt(modbits::LGUI, kbd::L));
    assert!(keys(&out).is_empty());

    let mut cfg = Config::default();
    cfg.lock = LockAction::Insert;
    let mut h = H::with(cfg);
    assert_eq!(keys(&h.send(s)), &[kbd::INSERT]);
}

#[test]
fn menu_key_is_application() {
    let mut h = H::new();
    let mut s = none();
    s.menu = true;
    assert_eq!(keys(&h.send(s)), &[kbd::APPLICATION]);
}

#[test]
fn eject_delete_and_insert() {
    let mut h = H::new();
    let mut s = none();
    s.eject = true;
    assert_eq!(keys(&h.send(s)), &[kbd::DELETE]);
    h.send(none());
    assert_eq!(keys(&h.send(s.with_fn(true))), &[kbd::INSERT]);
}

#[test]
fn eject_as_fn() {
    let mut cfg = Config::default();
    cfg.eject = EjectAction::Fn;
    let mut h = H::with(cfg);
    let mut s = ks(&[kbd::BACKSPACE]);
    s.eject = true;
    assert_eq!(keys(&h.send(s)), &[kbd::DELETE]);
}

#[test]
fn numpad_model_with_eject_defaults_eject_to_fn() {
    let m = crate::model::find(0x05AC, 0x026C).unwrap();
    assert_eq!(Config::default_for(m).eject, EjectAction::Fn);
    let m = crate::model::find(0x05AC, 0x029F).unwrap();
    assert_eq!(Config::default_for(m).eject, EjectAction::Delete);
}

// ---- Fn taps --------------------------------------------------------------------------------

fn tap_fn(h: &mut H, at: u32) -> (Outputs, Outputs) {
    let a = h.send_at(at, fnks(&[]));
    let b = h.send_at(at + 80, none());
    (a, b)
}

#[test]
fn double_tap_toggles_fn_lock_immediately_without_sleep_combo() {
    let mut cfg = Config::default();
    cfg.fn_double_tap = FnDoubleTap::ToggleFnLock;
    cfg.sleep_combo = false;
    let mut h = H::with(cfg);
    let (_, r1) = tap_fn(&mut h, 1000);
    assert_eq!(r1.notification, None);
    let (_, r2) = tap_fn(&mut h, 1200);
    assert_eq!(r2.notification, Some(Notification::FnLockChanged(FnMode::FKeysFirst)));
    assert_eq!(h.e.config().fn_mode, FnMode::FKeysFirst);
    assert_eq!(keys(&h.send(ks(&[0x41]))), &[0x41], "F-keys now come first");
}

#[test]
fn double_tap_deferred_when_sleep_combo_enabled() {
    let mut cfg = Config::default();
    cfg.fn_double_tap = FnDoubleTap::ToggleFnLock;
    let mut h = H::with(cfg);
    tap_fn(&mut h, 1000);
    let (_, r2) = tap_fn(&mut h, 1200);
    assert_eq!(r2.notification, None);
    let deadline = h.e.next_deadline().expect("deadline armed");
    assert_eq!(deadline, 1280 + TAP_WINDOW_MS);
    assert_eq!(h.tick_at(deadline - 1).notification, None);
    assert_eq!(
        h.tick_at(deadline).notification,
        Some(Notification::FnLockChanged(FnMode::FKeysFirst))
    );
    assert_eq!(h.e.next_deadline(), None);
}

#[test]
fn deferred_double_tap_fires_before_next_key() {
    let mut cfg = Config::default();
    cfg.fn_double_tap = FnDoubleTap::ToggleFnLock;
    let mut h = H::with(cfg);
    tap_fn(&mut h, 1000);
    tap_fn(&mut h, 1200);
    let out = h.send_at(1300, ks(&[0x41]));
    assert_eq!(out.notification, Some(Notification::FnLockChanged(FnMode::FKeysFirst)));
    assert_eq!(keys(&out), &[0x41], "key resolved with the new mode");
}

#[test]
fn double_tap_dictation_pulses_win_h() {
    let mut cfg = Config::default();
    cfg.fn_double_tap = FnDoubleTap::Dictation;
    cfg.sleep_combo = false;
    let mut h = H::with(cfg);
    tap_fn(&mut h, 1000);
    let (_, r2) = tap_fn(&mut h, 1200);
    assert_eq!(r2.pulse, Some(virt(modbits::LGUI, kbd::H)));
}

#[test]
fn slow_taps_do_not_count() {
    let mut cfg = Config::default();
    cfg.fn_double_tap = FnDoubleTap::ToggleFnLock;
    cfg.sleep_combo = false;
    let mut h = H::with(cfg);
    tap_fn(&mut h, 1000);
    let (_, r2) = tap_fn(&mut h, 2000);
    assert_eq!(r2.notification, None);
}

#[test]
fn fn_chord_is_not_a_tap() {
    let mut cfg = Config::default();
    cfg.fn_double_tap = FnDoubleTap::ToggleFnLock;
    cfg.sleep_combo = false;
    let mut h = H::with(cfg);
    h.send_at(1000, fnks(&[kbd::BACKSPACE]));
    h.send_at(1050, none());
    let (_, r2) = tap_fn(&mut h, 1150);
    assert_eq!(r2.notification, None);
}

#[test]
fn triple_tap_then_space_sleeps() {
    let mut cfg = Config::default();
    cfg.fn_double_tap = FnDoubleTap::ToggleFnLock;
    let mut h = H::with(cfg);
    tap_fn(&mut h, 1000);
    tap_fn(&mut h, 1200);
    tap_fn(&mut h, 1400);
    assert_eq!(h.e.next_deadline(), None, "double tap cancelled by third tap");
    let out = h.send_at(1600, ks(&[kbd::SPACE]));
    assert_eq!(out.system, system::SLEEP);
    assert!(keys(&out).is_empty(), "space is consumed");
    assert_eq!(out.notification, None);
    assert_eq!(h.e.config().fn_mode, FnMode::MediaFirst, "fn lock untouched");
    let out = h.send(none());
    assert_eq!(out.system, 0);
    assert!(out.system_changed);
}

#[test]
fn third_fn_held_plus_space_sleeps() {
    let mut h = H::new();
    tap_fn(&mut h, 1000);
    tap_fn(&mut h, 1200);
    h.send_at(1400, fnks(&[]));
    assert_eq!(h.send_at(1450, fnks(&[kbd::SPACE])).system, system::SLEEP);
}

#[test]
fn sleep_combo_disabled_or_expired() {
    let mut cfg = Config::default();
    cfg.sleep_combo = false;
    let mut h = H::with(cfg);
    for t in [1000, 1200, 1400] {
        tap_fn(&mut h, t);
    }
    assert_eq!(keys(&h.send_at(1600, ks(&[kbd::SPACE]))), &[kbd::SPACE]);

    let mut h = H::new();
    for t in [1000, 1200, 1400] {
        tap_fn(&mut h, t);
    }
    let late = 1480 + SLEEP_ARM_MS + 1;
    assert_eq!(keys(&h.send_at(late, ks(&[kbd::SPACE]))), &[kbd::SPACE]);
}

#[test]
fn timestamps_may_wrap() {
    let mut cfg = Config::default();
    cfg.fn_double_tap = FnDoubleTap::ToggleFnLock;
    cfg.sleep_combo = false;
    let mut h = H::with(cfg);
    let start = u32::MAX - 100;
    tap_fn(&mut h, start);
    let (_, r2) = tap_fn(&mut h, start.wrapping_add(200));
    assert!(r2.notification.is_some());
}

// ---- robustness -----------------------------------------------------------------------------

#[test]
fn rollover_report_keeps_previous_keys() {
    let mut h = H::new();
    h.send(ks(&[kbd::A, 0x05]));
    let out = h.send(ks(&[kbd::ERROR_ROLLOVER, kbd::LEFT_SHIFT]));
    assert_eq!(keys(&out), &[kbd::A, 0x05]);
    assert_eq!(out.keyboard.mods, modbits::LSHIFT);
}

#[test]
fn release_all_clears_virtual_devices() {
    let mut h = H::new();
    h.send(ks(&[0x45, 0x3C]));
    let mut out = Outputs::default();
    h.e.release_all(&mut out);
    assert!(consumer(&out).is_empty() && out.consumer_changed);
    assert_eq!(out.virt_keyboard, KbdReport::default());
    assert!(out.virt_keyboard_changed);
}

#[test]
fn multiple_consumer_keys() {
    let mut h = H::new();
    let out = h.send(ks(&[0x44, 0x45]));
    assert_eq!(consumer(&out), vec![consumer::VOLUME_DOWN, consumer::VOLUME_UP]);
    let out = h.send(ks(&[0x45]));
    assert_eq!(consumer(&out), vec![consumer::VOLUME_UP]);
}
