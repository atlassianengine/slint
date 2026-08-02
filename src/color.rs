use slint::Color;

pub fn accent_color(value: &str) -> Color {
    let (red, green, blue) = rgb_components(value);
    Color::from_rgb_u8(red, green, blue)
}

fn rgb_components(value: &str) -> (u8, u8, u8) {
    let parsed = u32::from_str_radix(value.trim_start_matches('#'), 16).unwrap_or(0x3b82f6);
    (
        ((parsed >> 16) & 0xff) as u8,
        ((parsed >> 8) & 0xff) as u8,
        (parsed & 0xff) as u8,
    )
}
