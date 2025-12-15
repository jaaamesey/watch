use font8x8::{self};
use minifb;
use watch_lib::{
    self, BoundingRect, Observable, RectUIElement, Signal, TextUIElement, UIContext, derived,
    derived2,
};

const SCREEN_WIDTH: u8 = 200;
const SCREEN_HEIGHT: u8 = 200;

fn main() {
    let test_signal = Signal::new(0);

    test_signal.subscribe(|n| {
        dbg!("value changed", n);
    });

    let derivation = derived(&test_signal, |n| n + 1);
    derivation.subscribe(|new| {
        dbg!("Derivation changed to", new);
    });

    let nested_derivation = derived(&derivation, |n| n * 2);
    nested_derivation.subscribe(|new| {
        dbg!("Nested derivation changed to", new);
    });
    nested_derivation.subscribe(|new| {
        dbg!("Nested derivation changed to 2", new);
    });

    let multi_derivation = derived2((&derivation, &nested_derivation), |(d, nd)| {
        d.to_string() + "-" + &nd.to_string()
    });
    multi_derivation.subscribe(|new| {
        dbg!("multi derivation changed to", new);
    });

    test_signal.set(20);
    test_signal.set(21);

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

    let counter = Signal::new(0);
    let toggled = derived(&counter, |c| {
        if c % 2 == 0 { "on" } else { "off" }.to_string()
    });

    let mut ui_context = UIContext::new(font8x8::unicode::BasicFonts::new());
    let root_id = ui_context.mount(RectUIElement::new(
        None,
        BoundingRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        },
        1,
    ));
    ui_context.mount(TextUIElement::new(
        &toggled,
        BoundingRect {
            x: 90,
            y: 76,
            width: 65,
            height: 20,
        },
        root_id,
    ));
    ui_context.mount(RectUIElement::new(
        None,
        BoundingRect {
            x: 80,
            y: 80,
            width: 100,
            height: 100,
        },
        1,
    ));
    ui_context.mount(TextUIElement::new(
        &toggled,
        BoundingRect {
            x: 100,
            y: 100,
            width: 100,
            height: 20,
        },
        root_id,
    ));
    let c = derived(&toggled, |_| "should be above".to_string());
    ui_context.mount(TextUIElement::new(
        &c,
        BoundingRect {
            x: 0,
            y: 100,
            width: 200,
            height: 20,
        },
        root_id,
    ));

    // set_pixel(&mut screen_buffer, 1, 1, 1);
    // set_rect(&mut screen_buffer, 10, 10, 100, 100, 1);

    // draw_text(&mut screen_buffer, &font, "ayy lmao", 1, 11, 0);

    while window.is_open() && !window.is_key_down(minifb::Key::Escape) {
        counter.set(counter.peek() + 1);
        ui_context.handle_draw_requests();
        let mut final_buffer: Vec<u32> = vec![0; SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize];
        for i in 0..final_buffer.len() {
            let color = get_pixel_by_index(ui_context.get_screen_buffer(), i);
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
