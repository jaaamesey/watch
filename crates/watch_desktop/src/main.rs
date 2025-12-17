use font8x8::{self};
use minifb;
use watch_lib::{
    self, BoundingRect, Observable, RectUIElement, SCREEN_HEIGHT, SCREEN_WIDTH, Signal,
    TextUIElement, UIContext, UIElement, derived, derived2,
};

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
    // TODO: figure out why it doesn't let this be inlined
    let parent_id = ui_context.mount(
        0,
        RectUIElement::new(
            BoundingRect {
                x: 80,
                y: 80,
                width: 100,
                height: 100,
            },
            1,
        ),
    );
    ui_context.mount(
        parent_id,
        TextUIElement::new(
            &toggled,
            BoundingRect {
                x: 10,
                y: 0,
                width: 64,
                height: 20,
            },
        ),
    );

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
