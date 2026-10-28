//! The remapping engine: turns decoded Apple keyboard reports into a rewritten keyboard report plus
//! virtual-device state (consumer / system control / synthesized shortcuts).
//!
//! The engine works on *physical key* press/release edges. When a key is pressed its output
//! [`Action`] is resolved once and latched; the release always undoes exactly that action. This keeps
//! output consistent when Fn or Shift change while a key is held (no stuck keys).

use crate::config::{ClearAction, Config, EjectAction, F6Action, FnDoubleTap, FnMode, LockAction, ModSource, ModTarget};
use crate::state::{KbdReport, KeyList, KeyState, Notification, Outputs, MAX_CONSUMER, MAX_KEYS};
use crate::usage::{consumer, kbd, modbits, system};

/// Physical key identifier: 0x00–0xFF are keyboard-page usages, higher values are Apple specials.
pub type PhysKey = u16;

pub mod phys {
    use super::PhysKey;
    pub const FN: PhysKey = 0x100;
    pub const EJECT: PhysKey = 0x101;
    pub const MENU: PhysKey = 0x102;
    pub const LOCK: PhysKey = 0x103;
}

/// Max gap between Fn taps, and max duration of a single tap, in milliseconds.
pub const TAP_WINDOW_MS: u32 = 350;
/// After three Fn taps, Space must follow within this time to trigger sleep.
pub const SLEEP_ARM_MS: u32 = 1500;

/// What a physical key does while held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    /// Keyboard usage in the real report (modifier usages set modifier bits).
    Key(u8),
    /// Keyboard usage in the real report that hides the given output modifier bits while held.
    KeyMasked { key: u8, suppress: u8 },
    Consumer(u16),
    System(u8),
    /// Shortcut held on the virtual keyboard.
    Combo { mods: u8, key: u8 },
    /// Acts as the Fn modifier.
    Fn,
}

const MAX_LATCH: usize = MAX_KEYS + 8 + 4;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct VirtState {
    keyboard: KbdReport,
    consumer: [u16; MAX_CONSUMER],
    system: u8,
}

/// Remapping engine. One instance per physical keyboard.
#[derive(Clone)]
pub struct Engine {
    cfg: Config,
    latch_phys: [PhysKey; MAX_LATCH],
    latch_action: [Action; MAX_LATCH],
    latch_len: usize,
    /// Non-modifier keys of the last report that was not a rollover report.
    last_keys: KeyList,
    prev_virt: VirtState,

    // Fn tap tracking.
    fn_down_clean: bool,
    fn_press_ms: u32,
    tap_count: u8,
    last_tap_ms: u32,
    /// Fn is currently held as the third press of a tap sequence.
    third_press: bool,
    pending_double_at: Option<u32>,
    sleep_armed_until: Option<u32>,
}

fn elapsed(now: u32, then: u32) -> u32 {
    now.wrapping_sub(then)
}

fn reached(now: u32, deadline: u32) -> bool {
    (now.wrapping_sub(deadline) as i32) >= 0
}

impl Engine {
    pub fn new(cfg: Config) -> Self {
        Engine {
            cfg,
            latch_phys: [0; MAX_LATCH],
            latch_action: [Action::None; MAX_LATCH],
            latch_len: 0,
            last_keys: KeyList::new(),
            prev_virt: VirtState::default(),
            fn_down_clean: false,
            fn_press_ms: 0,
            tap_count: 0,
            last_tap_ms: 0,
            third_press: false,
            pending_double_at: None,
            sleep_armed_until: None,
        }
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// Replaces the configuration. Keys already held keep their latched action until released.
    pub fn set_config(&mut self, cfg: Config) {
        self.cfg = cfg;
    }

    pub fn set_fn_mode(&mut self, mode: FnMode) {
        self.cfg.fn_mode = mode;
    }

    /// Next time [`Engine::tick`] must be called, if any timer is pending.
    pub fn next_deadline(&self) -> Option<u32> {
        self.pending_double_at
    }

    /// Processes one input report received at `now_ms` (monotonic milliseconds, may wrap).
    pub fn process(&mut self, input: &KeyState, now_ms: u32, out: &mut Outputs) {
        *out = Outputs::default();
        self.fire_due(now_ms, out);

        let mut cur = [0 as PhysKey; MAX_LATCH];
        let mut n = 0usize;
        let mut add = |p: PhysKey| {
            if n < MAX_LATCH {
                cur[n] = p;
                n += 1;
            }
        };
        for bit in 0..8u8 {
            if input.mods & (1 << bit) != 0 {
                add((kbd::LEFT_CTRL + bit) as PhysKey);
            }
        }
        if !input.is_rollover() {
            self.last_keys = input.keys;
        }
        for &k in self.last_keys.as_slice() {
            add(k as PhysKey);
        }
        if input.fn_key {
            add(phys::FN);
        }
        if input.eject {
            add(phys::EJECT);
        }
        if input.menu {
            add(phys::MENU);
        }
        if input.lock {
            add(phys::LOCK);
        }
        let cur = &cur[..n];

        // Releases.
        let mut i = 0;
        while i < self.latch_len {
            if cur.contains(&self.latch_phys[i]) {
                i += 1;
            } else {
                let was_fn = self.latch_action[i] == Action::Fn;
                self.remove_latch(i);
                if was_fn && !self.fn_held() {
                    self.on_fn_release(now_ms, out);
                }
            }
        }

        // Presses: modifier-like keys first so e.g. Fn and F8 arriving in the same report combine.
        for pass_modifiers in [true, false] {
            for &p in cur {
                if self.is_latched(p) || self.is_modifier_like(p) != pass_modifiers {
                    continue;
                }
                let mut action = self.resolve(p);
                if action == Action::Fn {
                    self.on_fn_press(now_ms);
                } else if let Some(over) = self.on_other_press(p, now_ms, out) {
                    action = over;
                } else {
                    // A deferred double-tap may have just changed the Fn mode.
                    action = self.resolve(p);
                }
                self.push_latch(p, action);
            }
        }

        self.build(out);
    }

    /// Fires expired timers. Call at [`Engine::next_deadline`]; only virtual outputs are meaningful.
    pub fn tick(&mut self, now_ms: u32, out: &mut Outputs) {
        *out = Outputs::default();
        self.fire_due(now_ms, out);
        self.build(out);
    }

    /// Releases everything (device removed / stopped). Returns the outputs needed to clear the
    /// virtual devices.
    pub fn release_all(&mut self, out: &mut Outputs) {
        *out = Outputs::default();
        self.latch_len = 0;
        self.last_keys.clear();
        self.tap_count = 0;
        self.third_press = false;
        self.pending_double_at = None;
        self.sleep_armed_until = None;
        self.build(out);
    }

    // ---- latch table -------------------------------------------------------------------------

    fn is_latched(&self, p: PhysKey) -> bool {
        self.latch_phys[..self.latch_len].contains(&p)
    }

    fn push_latch(&mut self, p: PhysKey, a: Action) {
        if self.latch_len < MAX_LATCH {
            self.latch_phys[self.latch_len] = p;
            self.latch_action[self.latch_len] = a;
            self.latch_len += 1;
        }
    }

    /// Order-preserving removal (keeps output ordering stable between reports).
    fn remove_latch(&mut self, i: usize) {
        self.latch_phys.copy_within(i + 1..self.latch_len, i);
        self.latch_action.copy_within(i + 1..self.latch_len, i);
        self.latch_len -= 1;
    }

    fn actions(&self) -> &[Action] {
        &self.latch_action[..self.latch_len]
    }

    fn fn_held(&self) -> bool {
        self.actions().contains(&Action::Fn)
    }

    /// Output-level modifier bits currently held through the real keyboard report.
    fn output_mods(&self) -> u8 {
        self.actions().iter().fold(0, |m, a| match *a {
            Action::Key(k) if kbd::is_modifier(k) => m | kbd::modifier_bit(k),
            _ => m,
        })
    }

    // ---- resolution --------------------------------------------------------------------------

    fn mod_source(p: PhysKey) -> Option<ModSource> {
        Some(match p {
            0xE0 => ModSource::LeftControl,
            0xE1 => ModSource::LeftShift,
            0xE2 => ModSource::LeftOption,
            0xE3 => ModSource::LeftCommand,
            0xE4 => ModSource::RightControl,
            0xE5 => ModSource::RightShift,
            0xE6 => ModSource::RightOption,
            0xE7 => ModSource::RightCommand,
            p if p == kbd::CAPS_LOCK as PhysKey => ModSource::CapsLock,
            phys::FN => ModSource::Fn,
            _ => return None,
        })
    }

    fn is_modifier_like(&self, p: PhysKey) -> bool {
        Self::mod_source(p).is_some() || (p == phys::EJECT && self.cfg.eject == EjectAction::Fn)
    }

    fn mod_action(&self, src: ModSource) -> Action {
        let right = src.is_right();
        let side = |l: u8, r: u8| Action::Key(if right { r } else { l });
        match self.cfg.modifier(src) {
            ModTarget::Ctrl => side(kbd::LEFT_CTRL, kbd::RIGHT_CTRL),
            ModTarget::Shift => side(kbd::LEFT_SHIFT, kbd::RIGHT_SHIFT),
            ModTarget::Alt => side(kbd::LEFT_ALT, kbd::RIGHT_ALT),
            ModTarget::Win => side(kbd::LEFT_GUI, kbd::RIGHT_GUI),
            ModTarget::AltGr => Action::Key(kbd::RIGHT_ALT),
            ModTarget::CapsLock => Action::Key(kbd::CAPS_LOCK),
            ModTarget::Fn => Action::Fn,
            ModTarget::Menu => Action::Key(kbd::APPLICATION),
            ModTarget::Insert => Action::Key(kbd::INSERT),
            ModTarget::Disabled => Action::None,
        }
    }

    /// Resolves the action for a newly pressed physical key given the current state.
    pub fn resolve(&self, p: PhysKey) -> Action {
        if let Some(src) = Self::mod_source(p) {
            return self.mod_action(src);
        }
        let fn_held = self.fn_held();
        match p {
            phys::EJECT => match self.cfg.eject {
                EjectAction::Delete => Action::Key(if fn_held { kbd::INSERT } else { kbd::DELETE }),
                EjectAction::Fn => Action::Fn,
                EjectAction::Disabled => Action::None,
            },
            phys::MENU => Action::Key(kbd::APPLICATION),
            phys::LOCK => match self.cfg.lock {
                LockAction::LockWorkstation => Action::Combo { mods: modbits::LGUI, key: kbd::L },
                LockAction::Insert => Action::Key(kbd::INSERT),
                LockAction::Disabled => Action::None,
            },
            p if p <= 0xFF => {
                let k = p as u8;
                if let Some(i) = kbd::f_row_index(k) {
                    self.resolve_f_row(i, k, fn_held)
                } else if let Some(i) = kbd::f_ext_index(k) {
                    match self.cfg.f_ext[i as usize] {
                        0 => Action::None,
                        u => Action::Key(u),
                    }
                } else if k == kbd::NUM_LOCK_CLEAR {
                    match self.cfg.clear {
                        ClearAction::NumLock => Action::Key(kbd::NUM_LOCK_CLEAR),
                        ClearAction::Delete => Action::Key(kbd::DELETE),
                        ClearAction::Backspace => Action::Key(kbd::BACKSPACE),
                        ClearAction::Disabled => Action::None,
                    }
                } else if fn_held {
                    Action::Key(fn_nav(k).unwrap_or(k))
                } else {
                    Action::Key(k)
                }
            }
            _ => Action::None,
        }
    }

    fn resolve_f_row(&self, i: u8, k: u8, fn_held: bool) -> Action {
        let mods = self.output_mods();
        if fn_held && mods & modbits::LSHIFT != 0 {
            match k {
                kbd::F11 => return Action::KeyMasked { key: kbd::PRINT_SCREEN, suppress: modbits::LSHIFT },
                kbd::F12 => return Action::KeyMasked { key: kbd::SCROLL_LOCK, suppress: modbits::LSHIFT },
                _ => {}
            }
        }
        let special = match self.cfg.fn_mode {
            FnMode::MediaFirst => !fn_held,
            FnMode::FKeysFirst => fn_held,
        };
        if !special {
            return Action::Key(k);
        }
        let shift = mods & modbits::ANY_SHIFT != 0;
        let win = |key| Action::Combo { mods: modbits::LGUI, key };
        match i {
            0 => Action::Consumer(consumer::BRIGHTNESS_DOWN),
            1 => Action::Consumer(consumer::BRIGHTNESS_UP),
            2 => win(kbd::TAB),
            3 => win(kbd::S),
            4 => win(kbd::H),
            5 => match self.cfg.f6 {
                F6Action::Sleep => Action::System(system::SLEEP),
                F6Action::DoNotDisturb => win(kbd::N),
                F6Action::None => Action::None,
            },
            6 => Action::Consumer(if shift { consumer::REWIND } else { consumer::PREV_TRACK }),
            7 => Action::Consumer(if shift { consumer::STOP } else { consumer::PLAY_PAUSE }),
            8 => Action::Consumer(if shift { consumer::FAST_FORWARD } else { consumer::NEXT_TRACK }),
            9 => Action::Consumer(consumer::MUTE),
            10 => Action::Consumer(consumer::VOLUME_DOWN),
            _ => Action::Consumer(consumer::VOLUME_UP),
        }
    }

    // ---- Fn taps -----------------------------------------------------------------------------

    fn on_fn_press(&mut self, now: u32) {
        if self.fn_held() {
            return; // a second Fn-role key; the first one drives tap tracking
        }
        self.fn_down_clean = true;
        self.fn_press_ms = now;
        // A further tap is starting: hold the pending double-tap until we know whether it is a triple.
        self.pending_double_at = None;
        self.third_press = self.tap_count >= 2 && elapsed(now, self.last_tap_ms) <= TAP_WINDOW_MS;
    }

    fn on_fn_release(&mut self, now: u32, out: &mut Outputs) {
        self.third_press = false;
        let is_tap = self.fn_down_clean && elapsed(now, self.fn_press_ms) <= TAP_WINDOW_MS;
        self.fn_down_clean = false;
        if !is_tap {
            self.tap_count = 0;
            return;
        }
        let gap_ok = elapsed(self.fn_press_ms, self.last_tap_ms) <= TAP_WINDOW_MS;
        self.tap_count = if self.tap_count > 0 && gap_ok { self.tap_count.saturating_add(1) } else { 1 };
        self.last_tap_ms = now;

        let double_enabled = self.cfg.fn_double_tap != FnDoubleTap::Off;
        if self.tap_count == 2 && double_enabled {
            if self.cfg.sleep_combo {
                // Might become a triple tap: defer.
                self.pending_double_at = Some(now.wrapping_add(TAP_WINDOW_MS));
            } else {
                self.fire_double(out);
                self.tap_count = 0;
            }
        } else if self.tap_count >= 3 && self.cfg.sleep_combo {
            self.pending_double_at = None;
            self.sleep_armed_until = Some(now.wrapping_add(SLEEP_ARM_MS));
        }
    }

    /// Bookkeeping for a non-Fn key press. Returns an overriding action (sleep combo) if any.
    fn on_other_press(&mut self, p: PhysKey, now: u32, out: &mut Outputs) -> Option<Action> {
        let fn_held = self.fn_held();
        if fn_held {
            self.fn_down_clean = false;
        }
        if self.pending_double_at.take().is_some() {
            self.fire_double(out);
        }
        let mut over = None;
        if self.cfg.sleep_combo && p == kbd::SPACE as PhysKey {
            let armed = self.sleep_armed_until.is_some_and(|until| !reached(now, until));
            if armed || (fn_held && self.third_press) {
                over = Some(Action::System(system::SLEEP));
            }
        }
        self.sleep_armed_until = None;
        self.third_press = false;
        if !fn_held {
            self.tap_count = 0;
        }
        over
    }

    fn fire_due(&mut self, now: u32, out: &mut Outputs) {
        if let Some(at) = self.pending_double_at {
            if reached(now, at) {
                self.pending_double_at = None;
                self.tap_count = 0;
                self.fire_double(out);
            }
        }
        if let Some(until) = self.sleep_armed_until {
            if reached(now, until) {
                self.sleep_armed_until = None;
            }
        }
    }

    fn fire_double(&mut self, out: &mut Outputs) {
        match self.cfg.fn_double_tap {
            FnDoubleTap::Off => {}
            FnDoubleTap::ToggleFnLock => {
                self.cfg.fn_mode = match self.cfg.fn_mode {
                    FnMode::MediaFirst => FnMode::FKeysFirst,
                    FnMode::FKeysFirst => FnMode::MediaFirst,
                };
                out.notification = Some(Notification::FnLockChanged(self.cfg.fn_mode));
            }
            FnDoubleTap::Dictation => {
                let mut keys = KeyList::new();
                keys.push(kbd::H);
                out.pulse = Some(KbdReport { mods: modbits::LGUI, keys });
            }
        }
    }

    // ---- output ------------------------------------------------------------------------------

    fn build(&mut self, out: &mut Outputs) {
        let mut kb = KbdReport::default();
        let mut suppress = 0u8;
        let mut virt = VirtState::default();
        let mut n_consumer = 0;
        for a in self.actions() {
            match *a {
                Action::Key(k) | Action::KeyMasked { key: k, .. } if kbd::is_modifier(k) => {
                    kb.mods |= kbd::modifier_bit(k);
                }
                Action::Key(k) => kb.keys.push(k),
                Action::KeyMasked { key, suppress: s } => {
                    kb.keys.push(key);
                    suppress |= s;
                }
                Action::Consumer(u) => {
                    if !virt.consumer.contains(&u) && n_consumer < MAX_CONSUMER {
                        virt.consumer[n_consumer] = u;
                        n_consumer += 1;
                    }
                }
                Action::System(s) => virt.system = s,
                Action::Combo { mods, key } => {
                    virt.keyboard.mods |= mods;
                    virt.keyboard.keys.push(key);
                }
                Action::Fn | Action::None => {}
            }
        }
        kb.mods &= !suppress;
        if kb.keys.overflowed() {
            kb.keys.clear();
            kb.keys.push(kbd::ERROR_ROLLOVER);
        }

        out.keyboard = kb;
        out.virt_keyboard = virt.keyboard;
        out.consumer = virt.consumer;
        out.system = virt.system;
        out.virt_keyboard_changed = virt.keyboard != self.prev_virt.keyboard;
        out.consumer_changed = virt.consumer != self.prev_virt.consumer;
        out.system_changed = virt.system != self.prev_virt.system;
        self.prev_virt = virt;
    }
}

/// Navigation/editing keys produced by Fn + key.
fn fn_nav(k: u8) -> Option<u8> {
    Some(match k {
        kbd::BACKSPACE => kbd::DELETE,
        kbd::ENTER => kbd::INSERT,
        kbd::LEFT => kbd::HOME,
        kbd::RIGHT => kbd::END,
        kbd::UP => kbd::PAGE_UP,
        kbd::DOWN => kbd::PAGE_DOWN,
        kbd::ESCAPE => kbd::PAUSE,
        kbd::P => kbd::PRINT_SCREEN,
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
