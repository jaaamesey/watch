use std::{cell::RefCell, rc::Rc};

use font8x8::{self, UnicodeFonts};
use minifb;
use watch_lib;

const SCREEN_WIDTH: u8 = 200;
const SCREEN_HEIGHT: u8 = 200;

struct Signal<T: Clone + Copy> {
    value: Rc<RefCell<T>>,
    listeners: Vec<Rc<RefCell<dyn FnMut(T)>>>,
}

impl<T: Copy> Signal<T> {
    pub fn set(&mut self, value: T) {
        *self.value.borrow_mut() = value;
    }
    pub fn derived<
        Computation: Clone + Copy,
        D0: Clone + Copy,
        D1: Clone + Copy,
        OD0: Observable<D0>,
        OD1: Observable<D1>,
    >(
        deps: (&mut OD0, &mut OD1),
        compute: &'static fn(D0, D1) -> Computation,
    ) {
        let ds = Rc::new(RefCell::new(DerivedSignal {
            cache: Some(compute(deps.0.peek(), deps.1.peek())),
            deps: (deps.0.peek(), deps.1.peek()),
            compute: |d| compute(d.0, d.1),
            listeners: Vec::new(),
        }));
        let ds_clone = ds.clone();
        deps.0.subscribe(move |new| {
            ds_clone.borrow_mut().deps.0 = new;
            ds_clone.borrow_mut().recompute();
        });
        let ds_clone = ds.clone();
        deps.1.subscribe(move |new| {
            ds_clone.borrow_mut().deps.1 = new;
            ds_clone.borrow_mut().recompute();
        });
    }
}

impl<T: Copy> Observable<T> for Signal<T> {
    fn peek(&self) -> T {
        *self.value.borrow()
    }
    fn subscribe<F: FnMut(T) + 'static>(&mut self, on_change: F) -> usize {
        self.listeners.push(Rc::new(RefCell::new(on_change)));
        self.listeners.len() - 1
    }
    fn unsubscribe(&mut self, id: usize) {
        // *self.listeners[id].borrow_mut() = || {};
    }
}

trait Observable<T> {
    fn peek(&self) -> T;
    fn subscribe<F: FnMut(T) + 'static>(&mut self, on_change: F) -> usize;
    fn unsubscribe(&mut self, id: usize);
}

struct DerivedSignal<Derivation: Clone + Copy, Deps: Clone + Copy, F: Fn(Deps) -> Derivation> {
    cache: Option<Derivation>,
    deps: Deps,
    compute: F,
    listeners: Vec<Rc<RefCell<dyn FnMut(Derivation)>>>,
}

impl<Derivation: Clone + Copy, Deps: Clone + Copy, F: Fn(Deps) -> Derivation>
    DerivedSignal<Derivation, Deps, F>
{
    pub fn recompute(&mut self) {
        let prev = self.cache;
        self.cache = Some((self.compute)(self.deps));
    }
}

struct TextUIElement<'a> {
    text: &'a Signal<&'a str>,
}

impl<'a> TextUIElement<'a> {
    // pub fn new(text: &'a Signal<&'a str>) -> Self {
    //     Self { text }
    // }
}

fn main() {
    let mut screen_buffer = [0 as u8; (SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize) / 8];

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

    let font = font8x8::unicode::BasicFonts::new();

    draw_text(&mut screen_buffer, &font, "ayy lmao", 1, 11, 0);

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

fn draw_text(
    buffer: &mut [u8],
    font: &font8x8::unicode::BasicFonts,
    text: &str,
    x_transform: u8,
    y_transform: u8,
    color: u8,
) {
    let char_width = 8;
    for (char_idx, c) in text.chars().enumerate() {
        let glyph = font.get(c).unwrap_or_default();
        for (row, row_bits) in glyph.iter().map(|byte| byte.reverse_bits()).enumerate() {
            let y = y_transform as usize + row;
            if y >= SCREEN_HEIGHT as usize {
                continue;
            }
            for col in 0..char_width {
                let x = x_transform as usize + char_idx * char_width + col;
                if x >= SCREEN_WIDTH as usize {
                    continue;
                }
                if (row_bits & (1 << (7 - col))) != 0 {
                    let byte_index = y * (SCREEN_WIDTH as usize / 8) + (x / 8);
                    let bit_index = 7 - (x % 8);
                    if color == 1 {
                        buffer[byte_index] |= 1 << bit_index;
                    } else {
                        buffer[byte_index] &= !(1 << bit_index);
                    }
                }
            }
        }
    }
}
