use std::sync::LazyLock;

use aphyn::*;

pub const MENUBAR_H: f32 = 26.0;
pub const TOOLBAR_H: f32 = 32.0;
pub const PANEL_HEADER_H: f32 = 26.0;
pub const GRIP_H: f32 = 12.0;
pub const ROW_H: f32 = 22.0;
pub const CONTROL_H: f32 = 24.0;
pub const CONTROL_H_SM: f32 = 20.0;
pub const INDENT: f32 = 14.0;
pub const FONT: f32 = 12.0;
pub const FONT_MONO: f32 = 11.0;
pub const FOLDOUT_ARROW_FONT: f32 = 16.0;
pub const PAD_X: f32 = 8.0;
pub const SCROLLBAR_W: f32 = 6.0;
pub const SCROLLBAR_HANDLE_W: f32 = 4.0;
pub const BORDER: f32 = 1.0;

pub static PAL: LazyLock<Palette> = LazyLock::new(Palette::dark);

pub fn srgb(hex: &str) -> Vec4<f32> {
    Vec4::from_array(parse_color_srgba_to_linear_f32(hex).unwrap())
}

// the opaque UI template discards fragments below the alpha threshold, so translucent design tokens are pre-blended;
// in sRGB, as the design tool (a browser) composites them
pub fn blend(top: Vec4<f32>, bottom: Vec4<f32>, alpha: f32) -> Vec4<f32> {
    let channel = |t: f32, b: f32| {
        let (t, b) = (linear_to_srgb(t), linear_to_srgb(b));
        srgb_to_linear(b + (t - b) * alpha)
    };
    Vec4::new(
        channel(top.x, bottom.x),
        channel(top.y, bottom.y),
        channel(top.z, bottom.z),
        1.0,
    )
}

fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

pub struct Palette {
    pub night_1000: Vec4<f32>,
    pub night_950: Vec4<f32>,
    pub night_600: Vec4<f32>,
    pub night_500: Vec4<f32>,
    pub night_400: Vec4<f32>,
    pub night_300: Vec4<f32>,

    pub bg_app: Vec4<f32>,
    pub bg_sunken: Vec4<f32>,
    pub bg_viewport: Vec4<f32>,

    pub surface_panel: Vec4<f32>,
    pub surface_raised: Vec4<f32>,
    pub surface_overlay: Vec4<f32>,
    pub surface_input: Vec4<f32>,
    pub surface_hover: Vec4<f32>,
    pub surface_active: Vec4<f32>,
    pub surface_selected: Vec4<f32>,
    pub surface_selected_strong: Vec4<f32>,

    pub border_subtle: Vec4<f32>,
    pub border_default: Vec4<f32>,
    pub border_strong: Vec4<f32>,
    pub border_focus: Vec4<f32>,

    pub text_1: Vec4<f32>,
    pub text_2: Vec4<f32>,
    pub text_3: Vec4<f32>,
    pub text_disabled: Vec4<f32>,
    pub text_on_accent: Vec4<f32>,

    pub accent: Vec4<f32>,
    pub accent_hover: Vec4<f32>,
    pub accent_fg: Vec4<f32>,
    pub highlight: Vec4<f32>,
    pub danger: Vec4<f32>,
    pub danger_soft_on_app: Vec4<f32>,
    pub danger_soft_on_hover: Vec4<f32>,
    pub accent_soft_on_panel: Vec4<f32>,
    pub warning: Vec4<f32>,
    pub warning_soft_on_panel: Vec4<f32>,

    pub axis_x: Vec4<f32>,
    pub axis_y: Vec4<f32>,
    pub axis_z: Vec4<f32>,

    pub bilberry_500: Vec4<f32>,
    pub bloom_400: Vec4<f32>,
    pub bloom_600: Vec4<f32>,
    pub juice_300: Vec4<f32>,
    pub resin_300: Vec4<f32>,
    pub leaf_200: Vec4<f32>,
    pub leaf_400: Vec4<f32>,

    pub transparent: Vec4<f32>,
}

impl Palette {
    fn dark() -> Self {
        let night_1000 = srgb("#0B0A10ff");
        let night_950 = srgb("#0E0D14ff");
        let night_900 = srgb("#14131Cff");
        let night_850 = srgb("#191822ff");
        let night_800 = srgb("#1F1E2Aff");
        let night_750 = srgb("#262533ff");
        let night_700 = srgb("#2E2D3Dff");
        let night_600 = srgb("#3A394Cff");
        let night_500 = srgb("#535268ff");
        let night_400 = srgb("#75748Cff");
        let night_300 = srgb("#9C9BB0ff");
        let night_100 = srgb("#E3E2EAff");

        let bilberry_300 = srgb("#9790D4ff");
        let bilberry_400 = srgb("#7870C4ff");
        let bilberry_500 = srgb("#5A51AEff");
        let rowan_400 = srgb("#D2675Bff");
        let resin_400 = srgb("#D4A458ff");

        Self {
            night_1000,
            night_950,
            night_600,
            night_500,
            night_400,
            night_300,

            bg_app: night_900,
            bg_sunken: night_950,
            bg_viewport: night_1000,

            surface_panel: night_850,
            surface_raised: night_800,
            surface_overlay: night_800,
            surface_input: night_950,
            surface_hover: night_750,
            surface_active: night_700,
            surface_selected: srgb("#2A2658ff"),
            surface_selected_strong: bilberry_500,

            border_subtle: srgb("#24232Fff"),
            border_default: night_700,
            border_strong: night_600,
            border_focus: bilberry_400,

            text_1: night_100,
            text_2: night_300,
            text_3: night_400,
            text_disabled: night_500,
            text_on_accent: srgb("#FFFFFFff"),

            accent: bilberry_500,
            accent_hover: bilberry_400,
            accent_fg: bilberry_300,
            highlight: srgb("#9D62BEff"),
            danger: rowan_400,
            danger_soft_on_app: blend(rowan_400, night_900, 0.16),
            danger_soft_on_hover: blend(rowan_400, night_750, 0.16),
            accent_soft_on_panel: blend(bilberry_400, night_850, 0.16),
            warning: resin_400,
            warning_soft_on_panel: blend(resin_400, night_850, 0.16),

            axis_x: rowan_400,
            axis_y: srgb("#6E9B67ff"),
            axis_z: srgb("#7287B0ff"),

            bilberry_500,
            bloom_400: srgb("#8FA3C7ff"),
            bloom_600: srgb("#586C93ff"),
            juice_300: srgb("#BD8DD6ff"),
            resin_300: srgb("#E0BA78ff"),
            leaf_200: srgb("#C3D8BFff"),
            leaf_400: srgb("#6E9B67ff"),

            transparent: Vec4::splat(0.0),
        }
    }
}
