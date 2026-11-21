//! 0.6.11: the colour scheme a window paints its engine defaults in.
//!
//! The engine draws a few things of its own when a node sets no colour: the glyphs of a text
//! node, a text input's text, placeholder, caret and selection, a scroll view's thumb, a
//! terminal's foreground, cursor and selection. Those defaults are not constants: they are
//! *roles* of the Material 3 baseline (seed `#6750A4`, standard contrast), and the window's
//! scheme (`light` or `dark`) decides which colour each role has. A node with no colour of its
//! own keeps no colour; the renderer resolves the role at paint time, so changing the scheme
//! repaints it with nothing running per frame. A node that sets a colour is never changed.
//!
//! [`SchemeColors`] is the whole role table (the 49 roles of the UX specification's
//! "Colour Roles" note). A window holds one resolved table, the baseline of its scheme with the
//! owner's per-role overrides on top.

use peniko::Color;

/// The two baselines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Scheme {
    #[default]
    Light,
    Dark,
}

impl Scheme {
    pub fn name(self) -> &'static str {
        match self {
            Scheme::Light => "light",
            Scheme::Dark => "dark",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "light" => Some(Scheme::Light),
            "dark" => Some(Scheme::Dark),
            _ => None,
        }
    }
}

const fn rgb(hex: u32) -> Color {
    Color::from_rgba8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 0xFF)
}

macro_rules! roles {
    ($($variant:ident, $name:literal, $light:literal, $dark:literal;)*) => {
        /// One named colour of the scheme.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(usize)]
        pub enum Role {
            $($variant,)*
        }

        impl Role {
            /// Every role, in the order of the specification's table.
            pub const ALL: &'static [Role] = &[$(Role::$variant,)*];

            /// The role's snake-case name, as Python spells it.
            pub fn name(self) -> &'static str {
                match self {
                    $(Role::$variant => $name,)*
                }
            }

            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Role::$variant),)*
                    _ => None,
                }
            }

            /// The baseline colour of this role in `scheme`.
            pub const fn baseline(self, scheme: Scheme) -> Color {
                match (self, scheme) {
                    $(
                        (Role::$variant, Scheme::Light) => rgb($light),
                        (Role::$variant, Scheme::Dark) => rgb($dark),
                    )*
                }
            }
        }
    };
}

roles! {
    Primary, "primary", 0x6750A4, 0xD0BCFF;
    OnPrimary, "on_primary", 0xFFFFFF, 0x381E72;
    PrimaryContainer, "primary_container", 0xEADDFF, 0x4F378B;
    OnPrimaryContainer, "on_primary_container", 0x21005D, 0xEADDFF;
    InversePrimary, "inverse_primary", 0xD0BCFF, 0x6750A4;
    PrimaryFixed, "primary_fixed", 0xEADDFF, 0xEADDFF;
    PrimaryFixedDim, "primary_fixed_dim", 0xD0BCFF, 0xD0BCFF;
    OnPrimaryFixed, "on_primary_fixed", 0x21005D, 0x21005D;
    OnPrimaryFixedVariant, "on_primary_fixed_variant", 0x4F378B, 0x4F378B;
    Secondary, "secondary", 0x625B71, 0xCCC2DC;
    OnSecondary, "on_secondary", 0xFFFFFF, 0x332D41;
    SecondaryContainer, "secondary_container", 0xE8DEF8, 0x4A4458;
    OnSecondaryContainer, "on_secondary_container", 0x1D192B, 0xE8DEF8;
    SecondaryFixed, "secondary_fixed", 0xE8DEF8, 0xE8DEF8;
    SecondaryFixedDim, "secondary_fixed_dim", 0xCCC2DC, 0xCCC2DC;
    OnSecondaryFixed, "on_secondary_fixed", 0x1D192B, 0x1D192B;
    OnSecondaryFixedVariant, "on_secondary_fixed_variant", 0x4A4458, 0x4A4458;
    Tertiary, "tertiary", 0x7D5260, 0xEFB8C8;
    OnTertiary, "on_tertiary", 0xFFFFFF, 0x492532;
    TertiaryContainer, "tertiary_container", 0xFFD8E4, 0x633B48;
    OnTertiaryContainer, "on_tertiary_container", 0x31111D, 0xFFD8E4;
    TertiaryFixed, "tertiary_fixed", 0xFFD8E4, 0xFFD8E4;
    TertiaryFixedDim, "tertiary_fixed_dim", 0xEFB8C8, 0xEFB8C8;
    OnTertiaryFixed, "on_tertiary_fixed", 0x31111D, 0x31111D;
    OnTertiaryFixedVariant, "on_tertiary_fixed_variant", 0x633B48, 0x633B48;
    Error, "error", 0xB3261E, 0xF2B8B5;
    OnError, "on_error", 0xFFFFFF, 0x601410;
    ErrorContainer, "error_container", 0xF9DEDC, 0x8C1D18;
    OnErrorContainer, "on_error_container", 0x410E0B, 0xF9DEDC;
    SurfaceDim, "surface_dim", 0xDED8E1, 0x141218;
    Surface, "surface", 0xFEF7FF, 0x141218;
    SurfaceTint, "surface_tint", 0x6750A4, 0xD0BCFF;
    SurfaceBright, "surface_bright", 0xFEF7FF, 0x3B383E;
    SurfaceContainerLowest, "surface_container_lowest", 0xFFFFFF, 0x0F0D13;
    SurfaceContainerLow, "surface_container_low", 0xF7F2FA, 0x1D1B20;
    SurfaceContainer, "surface_container", 0xF3EDF7, 0x211F26;
    SurfaceContainerHigh, "surface_container_high", 0xECE6F0, 0x2B2930;
    SurfaceContainerHighest, "surface_container_highest", 0xE6E0E9, 0x36343B;
    OnSurface, "on_surface", 0x1D1B20, 0xE6E0E9;
    OnSurfaceVariant, "on_surface_variant", 0x49454F, 0xCAC4D0;
    Outline, "outline", 0x79747E, 0x938F99;
    OutlineVariant, "outline_variant", 0xCAC4D0, 0x49454F;
    InverseSurface, "inverse_surface", 0x322F35, 0xE6E0E9;
    InverseOnSurface, "inverse_on_surface", 0xF5EFF7, 0x322F35;
    SurfaceVariant, "surface_variant", 0xE7E0EC, 0x49454F;
    Background, "background", 0xFEF7FF, 0x141218;
    OnBackground, "on_background", 0x1D1B20, 0xE6E0E9;
    Shadow, "shadow", 0x000000, 0x000000;
    Scrim, "scrim", 0x000000, 0x000000;
}

/// The resolved role table of one window: a baseline with overrides applied.
#[derive(Clone, Debug, PartialEq)]
pub struct SchemeColors {
    scheme: Scheme,
    colors: [Color; Role::ALL.len()],
}

impl Default for SchemeColors {
    fn default() -> Self {
        Self::baseline(Scheme::Light)
    }
}

impl SchemeColors {
    /// The published baseline of `scheme`, no overrides.
    pub fn baseline(scheme: Scheme) -> Self {
        let mut colors = [Color::TRANSPARENT; Role::ALL.len()];
        for &role in Role::ALL {
            colors[role as usize] = role.baseline(scheme);
        }
        Self { scheme, colors }
    }

    /// Which baseline this table started from.
    pub fn scheme(&self) -> Scheme {
        self.scheme
    }

    pub fn get(&self, role: Role) -> Color {
        self.colors[role as usize]
    }

    pub fn set(&mut self, role: Role, color: Color) {
        self.colors[role as usize] = color;
    }

    /// Every role with its colour, in the order of [`Role::ALL`].
    pub fn iter(&self) -> impl Iterator<Item = (Role, Color)> + '_ {
        Role::ALL.iter().map(|&role| (role, self.get(role)))
    }

    /// The colour of the text a node with no `fill` paints.
    pub fn text(&self) -> Color {
        self.get(Role::OnSurface)
    }

    /// A text input's placeholder with no `placeholder_fill`.
    pub fn placeholder(&self) -> Color {
        self.get(Role::OnSurfaceVariant)
    }

    /// A text input's caret with no `caret_color`, and a terminal's cursor.
    pub fn caret(&self) -> Color {
        self.get(Role::Primary)
    }

    /// The highlight behind selected text, with no `selection_fill`.
    pub fn selection(&self) -> Color {
        // UX vault (OQ, 2026-11-21): `primary` at 30 percent alpha. MD3 sources give primary;
        // 30 (not Compose's 40) keeps on_surface text at >= 4.5:1 over the highlight on every
        // surface role in both baselines (40 percent falls to 3.74:1 on dark surface_bright).
        self.get(Role::Primary).multiply_alpha(0.3)
    }

    /// A scroll view's thumb with no `scrollbar_fill`.
    pub fn scrollbar_thumb(&self) -> Color {
        self.get(Role::OnSurfaceVariant)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for &role in Role::ALL {
            assert_eq!(Role::from_name(role.name()), Some(role));
            assert!(seen.insert(role.name()), "{} twice", role.name());
        }
        assert_eq!(Role::ALL.len(), 49);
        assert_eq!(Role::from_name("nope"), None);
    }

    #[test]
    fn baselines_are_the_published_values() {
        let light = SchemeColors::baseline(Scheme::Light);
        let dark = SchemeColors::baseline(Scheme::Dark);
        assert_eq!(light.text(), Color::from_rgba8(0x1D, 0x1B, 0x20, 0xFF));
        assert_eq!(dark.text(), Color::from_rgba8(0xE6, 0xE0, 0xE9, 0xFF));
        assert_eq!(
            light.scrollbar_thumb(),
            Color::from_rgba8(0x49, 0x45, 0x4F, 0xFF)
        );
        assert_eq!(
            dark.scrollbar_thumb(),
            Color::from_rgba8(0xCA, 0xC4, 0xD0, 0xFF)
        );
        assert_eq!(light.scheme(), Scheme::Light);
        assert_eq!(dark.scheme(), Scheme::Dark);
    }

    #[test]
    fn a_tree_repaints_only_for_a_real_change_of_its_colours() {
        let mut tree = crate::Tree::new();
        assert_eq!(tree.scheme_colors(), &SchemeColors::baseline(Scheme::Light));
        let _ = tree.take_touched();
        assert!(!tree.set_scheme_colors(SchemeColors::baseline(Scheme::Light)));
        assert_eq!(tree.scheme_revision(), 0);
        assert!(!tree.take_touched().all, "no change, nothing touched");
        assert!(tree.set_scheme_colors(SchemeColors::baseline(Scheme::Dark)));
        assert_eq!(tree.scheme_revision(), 1);
        assert!(tree.take_touched().all, "a change touches every node");
    }

    #[test]
    fn an_override_changes_only_its_role() {
        let mut colors = SchemeColors::baseline(Scheme::Dark);
        let red = Color::from_rgba8(255, 0, 0, 255);
        colors.set(Role::OnSurface, red);
        assert_eq!(colors.text(), red);
        assert_eq!(
            colors.placeholder(),
            SchemeColors::baseline(Scheme::Dark).placeholder()
        );
    }
}
