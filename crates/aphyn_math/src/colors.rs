const fn parse_hex(ascii_char: u8) -> Option<u8> {
    Some(match ascii_char {
        b'0' => 0,
        b'1' => 1,
        b'2' => 2,
        b'3' => 3,
        b'4' => 4,
        b'5' => 5,
        b'6' => 6,
        b'7' => 7,
        b'8' => 8,
        b'9' => 9,
        b'a' | b'A' => 10,
        b'b' | b'B' => 11,
        b'c' | b'C' => 12,
        b'd' | b'D' => 13,
        b'e' | b'E' => 14,
        b'f' | b'F' => 15,
        _ => return None,
    })
}

pub const fn parse_color_rgb_u8(v: &str) -> Option<[u8; 3]> {
    const REQUIRED_BYTES_COUNT: usize = 6;

    let mut result = 0_u32;

    let mut bytes = v.as_bytes();

    if bytes.is_empty() {
        return None;
    }

    if bytes[0] == b'#' {
        bytes = bytes.split_at(1).1;
    }

    if bytes.len() < REQUIRED_BYTES_COUNT {
        return None;
    }

    let mut i = 0;
    while i < REQUIRED_BYTES_COUNT {
        let Some(byte) = parse_hex(bytes[i]) else {
            return None;
        };
        let byte = byte as u32;
        result <<= 4;
        result |= byte;
        i += 1;
    }

    let [_, r, g, b] = result.to_be_bytes();
    Some([r, g, b])
}

pub const fn parse_color_rgb_f32(v: &str) -> Option<[f32; 3]> {
    let Some(result_as_u8) = parse_color_rgb_u8(v) else {
        return None;
    };

    let mut result_as_f32 = [0.0_f32; 3];

    let mut i = 0;
    while i < 3 {
        result_as_f32[i] = result_as_u8[i] as f32 / 255.0;
        i += 1;
    }

    Some(result_as_f32)
}

pub const fn parse_color_rgba_u8(v: &str) -> Option<[u8; 4]> {
    const REQUIRED_BYTES_COUNT: usize = 8;
    let mut result = 0_u32;

    let mut bytes = v.as_bytes();

    if bytes.is_empty() {
        return None;
    }

    if bytes[0] == b'#' {
        bytes = bytes.split_at(1).1;
    }

    if bytes.len() < REQUIRED_BYTES_COUNT {
        return None;
    }

    let mut i = 0;
    while i < REQUIRED_BYTES_COUNT {
        let Some(byte) = parse_hex(bytes[i]) else {
            return None;
        };
        let byte = byte as u32;
        result <<= 4;
        result |= byte;
        i += 1;
    }

    Some(result.to_be_bytes())
}

pub const fn parse_color_rgba_f32(v: &str) -> Option<[f32; 4]> {
    let Some(result_as_u8) = parse_color_rgba_u8(v) else {
        return None;
    };

    let mut result_as_f32 = [0.0_f32; 4];

    let mut i = 0;
    while i < 4 {
        result_as_f32[i] = result_as_u8[i] as f32 / 255.0;
        i += 1;
    }

    Some(result_as_f32)
}
