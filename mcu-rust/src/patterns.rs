//! Red-only LED motion patterns and engine.

use smart_leds::RGB8;

use crate::state::AgentState;
use crate::theme::MotionTheme;

/// Number of LEDs on the strip (KnightRider reference used 12).
pub const LED_COUNT: usize = 8;

/// Default head color (KnightRider: red).
pub const BASE_COLOR: RGB8 = RGB8::new(255, 0, 0);

/// Fade retention per step (0..=255). Higher = slower trail fade.
pub const FADE_FACTOR: u8 = 180;

/// Default milliseconds between animation steps.
pub const ANIM_SPEED_MS: u32 = 35;

/// Tick for Fill Sweep / K.I.T.T. scanner (slower, more deliberate).
pub const SWEEP_SPEED_MS: u32 = 80;

/// Pattern identifiers (classic theme maps one state → one pattern).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternId {
    Breathing,
    CenterOut,
    FillSweep,
    KittScanner,
    Blink,
    Flash,
    Comet,
}

/// Owns the active pattern and shared pixel buffer.
pub struct LedEngine {
    theme: MotionTheme,
    state: AgentState,
    pattern: PatternId,
    pixels: [RGB8; LED_COUNT],
    /// Pattern-local phase / position.
    phase: u16,
    /// Direction for bounce/sweep (-1 / +1) encoded as i8.
    dir: i8,
    /// Extra counter (e.g. flash count).
    aux: u8,
}

impl LedEngine {
    pub const fn new(theme: MotionTheme) -> Self {
        let state = AgentState::Idle;
        Self {
            theme,
            state,
            pattern: theme.pattern_for(state),
            pixels: [RGB8::new(0, 0, 0); LED_COUNT],
            phase: 0,
            dir: 1,
            aux: 0,
        }
    }

    pub const fn theme(&self) -> MotionTheme {
        self.theme
    }

    pub const fn state(&self) -> AgentState {
        self.state
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    /// Frame delay for the active pattern (sweep motions use a slower tick).
    pub const fn anim_delay_ms(&self) -> u32 {
        match self.pattern {
            PatternId::FillSweep | PatternId::KittScanner => SWEEP_SPEED_MS,
            _ => ANIM_SPEED_MS,
        }
    }

    /// Switch motion theme if changed.
    pub fn set_theme(&mut self, theme: MotionTheme) {
        if theme == self.theme {
            return;
        }
        self.theme = theme;
        self.pattern = self.theme.pattern_for(self.state);
        self.reset_pattern();
    }

    /// Switch agent state (and thus pattern) if changed.
    pub fn set_state(&mut self, state: AgentState) {
        if state == self.state {
            return;
        }
        self.state = state;
        self.pattern = self.theme.pattern_for(state);
        self.reset_pattern();
    }

    fn reset_pattern(&mut self) {
        self.pixels = [RGB8::new(0, 0, 0); LED_COUNT];
        self.phase = 0;
        self.dir = 1;
        self.aux = 0;
    }

    /// Advance one animation frame; returns the pixel buffer to write.
    pub fn step(&mut self) -> &[RGB8; LED_COUNT] {
        let color = self.theme.color_for(self.state);
        match self.pattern {
            PatternId::Breathing => self.step_breathing(color),
            PatternId::CenterOut => self.step_center_out(color),
            PatternId::FillSweep => self.step_fill_sweep(color),
            PatternId::KittScanner => self.step_kitt_scanner(color),
            PatternId::Blink => self.step_blink(color),
            PatternId::Flash => self.step_flash(color),
            PatternId::Comet => self.step_comet(color),
        }
        &self.pixels
    }

    /// Boot / Wi-Fi wait: rainbow gradient slides back and forth along the strip.
    pub fn step_wifi_wait(&mut self) -> &[RGB8; LED_COUNT] {
        self.step_rainbow_bounce();
        &self.pixels
    }

    /// Reset animation state before showing the Wi-Fi wait rainbow.
    pub fn begin_wifi_wait(&mut self) {
        self.reset_pattern();
    }

    /// Apply the current agent state and reset pattern (e.g. after Wi-Fi comes up).
    pub fn sync_state(&mut self, state: AgentState) {
        self.state = state;
        self.pattern = self.theme.pattern_for(state);
        self.reset_pattern();
    }

    fn step_breathing(&mut self, color: RGB8) {
        // Triangle wave 0..255..0
        let p = self.phase as u8;
        let level = if p < 128 {
            p.saturating_mul(2)
        } else {
            255u8.saturating_sub(p.saturating_sub(128).saturating_mul(2))
        };
        let c = scale_color(color, level);
        self.pixels = [c; LED_COUNT];
        self.phase = self.phase.wrapping_add(2);
        if self.phase >= 256 {
            self.phase = 0;
        }
    }

    fn step_center_out(&mut self, color: RGB8) {
        fade_all(&mut self.pixels, FADE_FACTOR);
        // Radius expands 0 .. LED_COUNT/2 then contracts.
        let half = (LED_COUNT / 2) as u16;
        let cycle = half * 2;
        let t = self.phase % cycle;
        let radius = if t <= half { t } else { cycle - t };

        let mid_lo = (LED_COUNT / 2).saturating_sub(1);
        let mid_hi = LED_COUNT / 2;
        for i in 0..LED_COUNT {
            let dist = if i <= mid_lo {
                mid_lo - i
            } else {
                i - mid_hi
            };
            if dist as u16 == radius {
                self.pixels[i] = color;
            } else if dist as u16 + 1 == radius {
                self.pixels[i] = scale_color(color, 100);
            }
        }
        self.phase = self.phase.wrapping_add(1);
    }

    fn step_fill_sweep(&mut self, color: RGB8) {
        let fill_to = (self.phase as usize) % (LED_COUNT + 1);
        for i in 0..LED_COUNT {
            self.pixels[i] = if i < fill_to {
                color
            } else {
                RGB8::new(0, 0, 0)
            };
        }
        // Pause fully filled for a beat, then clear and restart.
        if fill_to >= LED_COUNT {
            self.aux = self.aux.saturating_add(1);
            if self.aux >= 8 {
                self.phase = 0;
                self.aux = 0;
            }
        } else {
            self.phase = self.phase.wrapping_add(1);
            self.aux = 0;
        }
    }

    fn step_kitt_scanner(&mut self, color: RGB8) {
        fade_all(&mut self.pixels, FADE_FACTOR);
        let pos = (self.phase as usize).min(LED_COUNT - 1);
        self.pixels[pos] = color;

        let next = pos as i8 + self.dir;
        if next >= (LED_COUNT as i8 - 1) {
            self.phase = (LED_COUNT - 1) as u16;
            self.dir = -1;
        } else if next <= 0 {
            self.phase = 0;
            self.dir = 1;
        } else {
            self.phase = next as u16;
        }
    }

    fn step_blink(&mut self, color: RGB8) {
        // ~350ms on / off at 35ms tick → toggle every 10 frames
        let on = (self.phase / 10) % 2 == 0;
        let c = if on {
            color
        } else {
            RGB8::new(0, 0, 0)
        };
        self.pixels = [c; LED_COUNT];
        self.phase = self.phase.wrapping_add(1);
    }

    fn step_flash(&mut self, color: RGB8) {
        // 6 rapid flashes then solid
        const FLASHES: u8 = 6;
        if self.aux < FLASHES * 2 {
            let on = self.aux % 2 == 0;
            let c = if on {
                color
            } else {
                RGB8::new(0, 0, 0)
            };
            self.pixels = [c; LED_COUNT];
            self.aux = self.aux.saturating_add(1);
        } else {
            self.pixels = [color; LED_COUNT];
        }
    }

    fn step_comet(&mut self, color: RGB8) {
        fade_all(&mut self.pixels, 140); // longer trail
        let pos = (self.phase as usize) % LED_COUNT;
        self.pixels[pos] = color;
        // Soft secondary head
        let prev = if pos == 0 { LED_COUNT - 1 } else { pos - 1 };
        self.pixels[prev] = scale_color(color, 160);
        self.phase = self.phase.wrapping_add(1);
    }

    fn step_rainbow_bounce(&mut self) {
        // Spread hues across the strip; `phase` is the sliding offset (0..=255).
        const HUE_SPACING: u8 = (256 / LED_COUNT) as u8;
        for i in 0..LED_COUNT {
            let hue = (self.phase as u8).wrapping_add(HUE_SPACING.wrapping_mul(i as u8));
            self.pixels[i] = hsv_to_rgb(hue, 255, 255);
        }

        const STEP: i16 = 4;
        let next = self.phase as i16 + i16::from(self.dir) * STEP;
        if next >= 255 {
            self.phase = 255;
            self.dir = -1;
        } else if next <= 0 {
            self.phase = 0;
            self.dir = 1;
        } else {
            self.phase = next as u16;
        }
    }
}

/// 8-bit HSV → RGB (full sat/value yields a classic rainbow wheel).
fn hsv_to_rgb(h: u8, s: u8, v: u8) -> RGB8 {
    if s == 0 {
        return RGB8::new(v, v, v);
    }
    let region = h / 43;
    let remainder = (h - region * 43) * 6;
    let p = scale8(v, 255u8.saturating_sub(s));
    let q = scale8(v, 255u8.saturating_sub(scale8(s, remainder)));
    let t = scale8(
        v,
        255u8.saturating_sub(scale8(s, 255u8.saturating_sub(remainder))),
    );
    match region {
        0 => RGB8::new(v, t, p),
        1 => RGB8::new(q, v, p),
        2 => RGB8::new(p, v, t),
        3 => RGB8::new(p, q, v),
        4 => RGB8::new(t, p, v),
        _ => RGB8::new(v, p, q),
    }
}

#[inline]
fn scale8(value: u8, scale: u8) -> u8 {
    ((u16::from(value) * u16::from(scale)) / 255) as u8
}

#[inline]
fn scale_color(c: RGB8, scale: u8) -> RGB8 {
    RGB8::new(scale8(c.r, scale), scale8(c.g, scale), scale8(c.b, scale))
}

fn fade_all(pixels: &mut [RGB8; LED_COUNT], factor: u8) {
    for pixel in pixels {
        pixel.r = scale8(pixel.r, factor);
        pixel.g = scale8(pixel.g, factor);
        pixel.b = scale8(pixel.b, factor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_switches_pattern_with_state() {
        let mut eng = LedEngine::new(MotionTheme::Classic);
        assert_eq!(eng.pattern(), PatternId::Breathing);
        eng.set_state(AgentState::ExecutingTool);
        assert_eq!(eng.pattern(), PatternId::KittScanner);
        eng.set_state(AgentState::Error);
        assert_eq!(eng.pattern(), PatternId::Comet);
    }

    #[test]
    fn engine_theme_switching() {
        let mut eng = LedEngine::new(MotionTheme::Classic);
        assert_eq!(eng.theme(), MotionTheme::Classic);
        eng.set_state(AgentState::ExecutingTool);
        assert_eq!(eng.pattern(), PatternId::KittScanner);

        eng.set_theme(MotionTheme::Colorful);
        assert_eq!(eng.theme(), MotionTheme::Colorful);
        assert_eq!(eng.pattern(), PatternId::KittScanner);
    }

    #[test]
    fn sweep_patterns_use_slower_delay() {
        let mut eng = LedEngine::new(MotionTheme::Classic);
        assert_eq!(eng.anim_delay_ms(), ANIM_SPEED_MS);

        eng.set_state(AgentState::Generating);
        assert_eq!(eng.pattern(), PatternId::FillSweep);
        assert_eq!(eng.anim_delay_ms(), SWEEP_SPEED_MS);

        eng.set_state(AgentState::ExecutingTool);
        assert_eq!(eng.pattern(), PatternId::KittScanner);
        assert_eq!(eng.anim_delay_ms(), SWEEP_SPEED_MS);

        eng.set_state(AgentState::Idle);
        assert_eq!(eng.anim_delay_ms(), ANIM_SPEED_MS);
    }

    #[test]
    fn steps_do_not_panic() {
        for theme in [MotionTheme::Classic, MotionTheme::Colorful] {
            let mut eng = LedEngine::new(theme);
            for state in [
                AgentState::Idle,
                AgentState::Thinking,
                AgentState::Generating,
                AgentState::ExecutingTool,
                AgentState::WaitingInput,
                AgentState::Done,
                AgentState::Error,
            ] {
                eng.set_state(state);
                for _ in 0..64 {
                    let frame = eng.step();
                    assert_eq!(frame.len(), LED_COUNT);
                }
            }
        }
    }

    #[test]
    fn wifi_wait_rainbow_bounces() {
        let mut eng = LedEngine::new(MotionTheme::Classic);
        let mut saw_nonzero = false;
        for _ in 0..200 {
            let frame = eng.step_wifi_wait();
            assert_eq!(frame.len(), LED_COUNT);
            if frame.iter().any(|c| c.r | c.g | c.b != 0) {
                saw_nonzero = true;
            }
        }
        assert!(saw_nonzero);
        eng.sync_state(AgentState::Idle);
        assert_eq!(eng.pattern(), PatternId::Breathing);
    }
}
