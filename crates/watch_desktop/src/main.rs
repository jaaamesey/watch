use font8x8::{self, UnicodeFonts};
use minifb;
use watch_lib;

const SCREEN_WIDTH: u8 = 200;
const SCREEN_HEIGHT: u8 = 200;

fn main() {
    let mut screen_buffer = [0 as u8; (SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize) / 8];
    dbg!(&screen_buffer);
    dbg!(watch_lib::add(1, 2));
    // let mut count = 0;
    // loop {
    //     count += 1;
    //     dbg!(count);
    //     thread::sleep(Duration::from_secs(1));
    // }

    let mut window = minifb::Window::new(
        "WATCH DEBUG SCREEN",
        SCREEN_WIDTH as usize,
        SCREEN_HEIGHT as usize,
        minifb::WindowOptions::default(),
    )
    .unwrap_or_else(|e| {
        panic!("{}", e);
    });

    // Mimic low refresh rate of e-ink
    window.set_target_fps(3);

    set_pixel(&mut screen_buffer, 1, 1, 1);
    set_rect(&mut screen_buffer, 10, 10, 100, 100, 1);

    let text_bitmap = render_text("some text ayy");
    let x_transform = 108;
    let y_transform = 0;
    for (byte_idx, byte) in text_bitmap.iter().enumerate() {
        let x = byte_idx;
        let y = byte_idx % 8;
        let buffer_byte_index = (x_transform / 8)
            + ((y_transform / 8) * SCREEN_WIDTH as usize)
            + y * (SCREEN_WIDTH as usize / 8)
            + (x / 8);
        if buffer_byte_index < screen_buffer.len()
            && ((x / 8 + x_transform / 8) < SCREEN_WIDTH as usize / 8)
        {
            screen_buffer[buffer_byte_index] |= *byte;
        }
    }

    while window.is_open() && !window.is_key_down(minifb::Key::Escape) {
        let mut final_buffer: Vec<u32> = vec![0; SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize];
        for i in 0..final_buffer.len() {
            let color = get_pixel_by_index(&screen_buffer, i);
            final_buffer[i] = if color == 0 { 0 } else { u32::MAX };
        }
        window
            .update_with_buffer(&final_buffer, SCREEN_WIDTH.into(), SCREEN_HEIGHT.into())
            .unwrap();
    }
}

fn get_pixel_by_index(buffer: &[u8], i: usize) -> u8 {
    if i >= SCREEN_HEIGHT as usize * SCREEN_WIDTH as usize {
        return 0;
    }

    let x = i % (SCREEN_WIDTH as usize);
    let y = i / (SCREEN_WIDTH as usize);

    let byte_index = (y * (SCREEN_WIDTH as usize / 8) + (x / 8)) as usize;
    let bit_index = 7 - (x % 8);

    if byte_index >= buffer.len() {
        return 0;
    }

    let byte_value = buffer[byte_index];
    if (byte_value >> bit_index) & 1 == 1 {
        1
    } else {
        0
    }
}

fn set_rect(buffer: &mut [u8], rect_x: u8, rect_y: u8, rect_width: u8, rect_height: u8, color: u8) {
    let fill_color = if color == 0 { 0 } else { 1 };

    for y in rect_y..(rect_y + rect_height) {
        if y >= SCREEN_HEIGHT {
            break;
        }
        for x in rect_x..(rect_x + rect_width) {
            if x >= SCREEN_WIDTH {
                break;
            }
            set_pixel(buffer, x, y, fill_color);
        }
    }
}

fn set_pixel(buffer: &mut [u8], x: u8, y: u8, color: u8) {
    if x >= SCREEN_WIDTH || y >= SCREEN_HEIGHT {
        return;
    }
    let byte_index = y as usize * (SCREEN_WIDTH as usize / 8) + (x as usize / 8);
    let bit_index = 7 - (x % 8);

    if color == 1 {
        buffer[byte_index] |= 1 << bit_index;
    } else {
        buffer[byte_index] &= !(1 << bit_index);
    }
}

// TODO: does this get optimised? could we do an iterator?
fn render_text(str: &str) -> Vec<u8> {
    let font = font8x8::unicode::BasicFonts::new();
    let char_width = 8;
    let buffer_width = str.len() * char_width;

    // TODO: figure out how to do this without vec
    let mut buffer = vec![0 as u8; buffer_width];

    for (c_idx, char) in str.chars().enumerate() {
        let char_buffer = font.get(char).unwrap_or_default();
        for (rendered_byte_idx, rendered_byte) in char_buffer.iter().enumerate() {
            buffer[c_idx * char_width + rendered_byte_idx] |= rendered_byte.reverse_bits();
        }
    }

    return buffer;
}
