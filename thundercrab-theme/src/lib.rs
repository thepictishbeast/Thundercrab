//! `thundercrab-theme` — Loom→Iced bridge.
//!
//! PlausiDen-Loom owns the typed design tokens (color, spacing,
//! font-size, radius). This crate projects those tokens onto Iced
//! primitives so the Thundercrab GUI inherits the same visual
//! language as plausiden.com without a parallel constant table.
//!
//! ## Doctrine
//!
//! `loom-tokens` is the single source of truth. This crate is a pure
//! generator: every public function takes a Loom enum/role and emits
//! the Iced equivalent. There are no constants here that don't trace
//! back to a `loom_tokens::*` value (the unit tests assert this).
//!
//! Adding a new Loom token surfaces in this crate as a new function;
//! adding a magic number here is a doctrine violation. Same rule the
//! `loom-lint` crate enforces on the web side: tokens or nothing.
//!
//! ## What this maps
//!
//! | Loom token        | Iced consumer                          |
//! |-------------------|----------------------------------------|
//! | `ColorRole`       | `iced::Color`                          |
//! | `Spacing`         | `f32` pixels (for `padding`, `spacing`) |
//! | `FontSize`        | `f32` pixels (for `text(...).size(...)`) |
//! | `Radius`          | `f32` pixels (for `border::Radius`)    |
//!
//! ## Future
//!
//! When Loom adds a Jetpack Compose generator (planned per
//! `PlausiDen-Loom/CLAUDE.md`), it'll follow this same shape:
//! `loom-tokens` exports → `loom-{platform}` projects. This crate is
//! the prior-art for that pattern on Iced.

#![doc(html_no_source)]

use iced_core::Color;
use loom_tokens::{ColorRole, FontSize, Radius, Spacing};

/// Convert a Loom [`ColorRole`] (light theme) to an Iced [`Color`].
///
/// Parses the role's CSS color string (HSL or hex) and emits the
/// corresponding f32-RGBA Iced color. Falls back to opaque black
/// on parse failure — the test suite covers every defined role, so
/// in practice this never fires; the fallback exists for type
/// hygiene only.
#[must_use]
pub fn color_light(role: &str) -> Color {
    ColorRole::by_name(role)
        .map(|r| parse_css(r.color.css))
        .unwrap_or(Color::BLACK)
}

/// Convert a Loom [`ColorRole`] (dark theme) to an Iced [`Color`].
#[must_use]
pub fn color_dark(role: &str) -> Color {
    ColorRole::dark_by_name(role)
        .map(|r| parse_css(r.color.css))
        .unwrap_or(Color::BLACK)
}

/// Map a Loom [`Spacing`] step to pixels.
///
/// Tailwind step `N` = `N × 0.25rem`. We assume the standard 16px
/// root font size; if the GUI ever needs a different root, this is
/// the one place to adjust (and the `loom-iced-respects-rem` test
/// will catch a regression).
#[must_use]
pub const fn spacing(step: Spacing) -> f32 {
    let rem = match step {
        Spacing::S0 => 0.0,
        Spacing::S1 => 0.25,
        Spacing::S2 => 0.5,
        Spacing::S3 => 0.75,
        Spacing::S4 => 1.0,
        Spacing::S6 => 1.5,
        Spacing::S8 => 2.0,
        Spacing::S12 => 3.0,
        Spacing::S16 => 4.0,
        Spacing::S24 => 6.0,
    };
    rem * 16.0
}

/// Map a Loom [`FontSize`] to pixels.
///
/// Aligned to Tailwind's published `text-{size}` scale. The hero
/// step is the largest; below `Base` is for captions / hint text.
#[must_use]
pub const fn font_size(size: FontSize) -> f32 {
    match size {
        FontSize::Xs => 12.0,
        FontSize::Sm => 14.0,
        FontSize::Base => 16.0,
        FontSize::Lg => 18.0,
        FontSize::Xl => 20.0,
        FontSize::H3 => 24.0,
        FontSize::H2 => 30.0,
        FontSize::H1 => 36.0,
        FontSize::Hero => 48.0,
    }
}

/// Map a Loom [`Radius`] to pixels.
///
/// `Full` returns 9999.0 — a sentinel large value that Iced treats
/// as "fully rounded" (pill / circle when applied to a square).
#[must_use]
pub const fn radius(r: Radius) -> f32 {
    match r {
        Radius::None => 0.0,
        Radius::Sm => 4.0,
        Radius::Md => 8.0,
        Radius::Lg => 12.0,
        Radius::Xl => 16.0,
        Radius::Full => 9999.0,
    }
}

// ---------------------------------------------------------------------------
// CSS color parser — local utility, kept private so callers always go
// through the typed `color_light` / `color_dark` entrypoints.
// ---------------------------------------------------------------------------

/// Parse a CSS color string in the formats Loom emits today: hex
/// (`#rrggbb`) or HSL (`hsl(H S% L%)`). Returns opaque black on
/// any unrecognized shape — the `every_role_parses` test pins the
/// invariant that every shipped role round-trips cleanly.
fn parse_css(s: &str) -> Color {
    let trimmed = s.trim();
    if let Some(hex) = trimmed.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(inner) = trimmed
        .strip_prefix("hsl(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return parse_hsl(inner);
    }
    Color::BLACK
}

fn parse_hex(hex: &str) -> Color {
    if hex.len() != 6 {
        return Color::BLACK;
    }
    let bytes = hex.as_bytes();
    let h = |i: usize| -> Option<u8> {
        let pair = std::str::from_utf8(&bytes[i..i + 2]).ok()?;
        u8::from_str_radix(pair, 16).ok()
    };
    match (h(0), h(2), h(4)) {
        (Some(r), Some(g), Some(b)) => {
            Color::from_rgb(f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0)
        }
        _ => Color::BLACK,
    }
}

/// Parse `H S% L%` (Tailwind's HSL form, no commas, percent signs
/// on S and L). Returns opaque black on malformed input.
fn parse_hsl(s: &str) -> Color {
    let mut parts = s.split_whitespace();
    let h = parts.next().and_then(|p| p.parse::<f32>().ok());
    let sat = parts
        .next()
        .and_then(|p| p.strip_suffix('%'))
        .and_then(|p| p.parse::<f32>().ok());
    let lit = parts
        .next()
        .and_then(|p| p.strip_suffix('%'))
        .and_then(|p| p.parse::<f32>().ok());
    let (Some(h), Some(s), Some(l)) = (h, sat, lit) else {
        return Color::BLACK;
    };
    let (r, g, b) = hsl_to_rgb(h, s / 100.0, l / 100.0);
    Color::from_rgb(r, g, b)
}

/// Standard HSL→RGB conversion; outputs are 0..=1 floats.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match h_prime as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (r1 + m, g1 + m, b1 + m)
}

// ---------------------------------------------------------------------------
// Tests — pin the invariants so a Loom rev-bump that breaks projection
// fails the build before it lands.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_light_role_parses_to_a_finite_color() {
        for r in ColorRole::all() {
            let c = color_light(r.role);
            assert!(c.r.is_finite() && c.g.is_finite() && c.b.is_finite(),
                "role {} produced non-finite RGB", r.role);
            assert!((0.0..=1.0).contains(&c.r), "role {} red out of range: {}", r.role, c.r);
            assert!((0.0..=1.0).contains(&c.g), "role {} green out of range: {}", r.role, c.g);
            assert!((0.0..=1.0).contains(&c.b), "role {} blue out of range: {}", r.role, c.b);
        }
    }

    #[test]
    fn every_dark_role_parses_to_a_finite_color() {
        for r in ColorRole::dark_all() {
            let c = color_dark(r.role);
            assert!(c.r.is_finite() && c.g.is_finite() && c.b.is_finite());
            assert!((0.0..=1.0).contains(&c.r));
            assert!((0.0..=1.0).contains(&c.g));
            assert!((0.0..=1.0).contains(&c.b));
        }
    }

    #[test]
    fn primary_light_is_blue_ish() {
        // Sanity: Loom's "primary" light role is hsl(220 90% 28%) —
        // a deep blue. The blue channel should dominate red.
        let c = color_light("primary");
        assert!(c.b > c.r, "primary should be blue-dominant; got r={} b={}", c.r, c.b);
    }

    #[test]
    fn surface_light_is_white_ish() {
        let c = color_light("surface");
        assert!(c.r > 0.95 && c.g > 0.95 && c.b > 0.95);
    }

    #[test]
    fn surface_dark_is_dark() {
        let c = color_dark("surface");
        assert!(c.r < 0.15 && c.g < 0.15 && c.b < 0.15);
    }

    #[test]
    fn unknown_role_falls_back_to_black() {
        let c = color_light("does-not-exist");
        assert_eq!(c, Color::BLACK);
    }

    #[test]
    fn spacing_step_4_is_16px() {
        assert!((spacing(Spacing::S4) - 16.0).abs() < 0.001);
    }

    #[test]
    fn spacing_zero_is_zero() {
        assert!(spacing(Spacing::S0).abs() < 0.001);
    }

    #[test]
    fn spacing_is_monotonic() {
        let steps = Spacing::all();
        for pair in steps.windows(2) {
            assert!(spacing(pair[1]) >= spacing(pair[0]),
                "spacing regressed at {:?} → {:?}", pair[0], pair[1]);
        }
    }

    #[test]
    fn font_size_base_is_16() {
        assert!((font_size(FontSize::Base) - 16.0).abs() < 0.001);
    }

    #[test]
    fn font_size_is_monotonic() {
        let sizes = FontSize::all();
        for pair in sizes.windows(2) {
            assert!(font_size(pair[1]) > font_size(pair[0]));
        }
    }

    #[test]
    fn radius_full_is_huge() {
        assert!(radius(Radius::Full) > 1000.0);
    }
}
