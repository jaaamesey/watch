#![no_std]
extern crate alloc;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use font8x8::UnicodeFonts;
use hashbrown::HashSet;

pub const SCREEN_WIDTH: u8 = 200;
pub const SCREEN_HEIGHT: u8 = 200;

// TODO: safely handle unwraps
struct SignalData<T: Clone> {
    value: T,
    listeners: Vec<Option<Rc<RefCell<dyn FnMut(T)>>>>,
}

#[derive(Clone)]
pub struct Signal<T: Clone> {
    data: Rc<RefCell<SignalData<T>>>,
}

impl<T: Copy + PartialEq> Signal<T> {
    pub fn set(&self, value: T) {
        let prev = self.peek();
        if value == prev {
            return;
        }
        let mut data = self.data.borrow_mut();
        data.value = value;
        for i in 0..data.listeners.len() {
            if let Some(listener) = &data.listeners[i] {
                (listener.borrow_mut())(value);
            }
        }
    }
    pub fn new(initial_value: T) -> Signal<T> {
        Signal {
            data: Rc::new(RefCell::new(SignalData {
                value: initial_value,
                listeners: Vec::with_capacity(4),
            })),
        }
    }
}

impl<T: Copy> Observable<T> for Signal<T> {
    fn peek(&self) -> T {
        self.data.borrow().value
    }
    fn subscribe<F: FnMut(T) + 'static>(&self, on_change: F) -> usize {
        let mut data = self.data.borrow_mut();
        // TODO: is there a more performant way to fill old gaps? remember, index matters - maybe this should be a hashmap instead
        let chosen_index = data
            .listeners
            .iter()
            .enumerate()
            .rev()
            .find(|(_, v)| v.is_none())
            .map(|i| i.0);
        if let Some(i) = chosen_index {
            data.listeners[i] = Some(Rc::new(RefCell::new(on_change)));
        } else {
            data.listeners.push(Some(Rc::new(RefCell::new(on_change))));
        }
        data.listeners.len() - 1
    }
    fn unsubscribe(&self, id: usize) {
        let mut data = self.data.borrow_mut();
        data.listeners[id] = None;
    }
}

pub fn derived<
    Computation: Clone + PartialEq + 'static,
    Compute: Fn(D0) -> Computation + 'static,
    D0: Clone + 'static,
    OD0: Observable<D0>,
>(
    dep0: &OD0,
    compute: Compute,
) -> DerivedSignal<Computation, D0, Compute> {
    let ds = DerivedSignal {
        data: Rc::new(RefCell::new(DerivedSignalData {
            cache: None,
            deps: dep0.peek(),
            compute: compute,
            listeners: Vec::with_capacity(4),
        })),
    };
    let ds_clone = ds.data.clone();
    dep0.subscribe(move |new| {
        let mut borrowed = ds_clone.borrow_mut();
        borrowed.deps = new;
        borrowed.maybe_recompute();
    });
    ds
}

pub fn derived2<
    Computation: Clone + PartialEq + 'static,
    Compute: Fn((D0, D1)) -> Computation + 'static,
    D0: Clone + 'static,
    D1: Clone + 'static,
    OD0: Observable<D0>,
    OD1: Observable<D1>,
>(
    deps: (&OD0, &OD1),
    compute: Compute,
) -> DerivedSignal<Computation, (D0, D1), Compute> {
    let ds = DerivedSignal {
        data: Rc::new(RefCell::new(DerivedSignalData {
            cache: None,
            deps: (deps.0.peek(), deps.1.peek()),
            compute,
            listeners: Vec::with_capacity(4),
        })),
    };
    let ds_clone = ds.data.clone();
    deps.0.subscribe(move |new| {
        let mut borrowed = ds_clone.borrow_mut();
        borrowed.deps.0 = new;
        borrowed.maybe_recompute();
    });
    let ds_clone = ds.data.clone();
    deps.1.subscribe(move |new| {
        let mut borrowed = ds_clone.borrow_mut();
        borrowed.deps.1 = new;
        borrowed.maybe_recompute();
    });
    ds
}

pub trait Observable<T> {
    fn peek(&self) -> T;
    fn subscribe<F: FnMut(T) + 'static>(&self, on_change: F) -> usize;
    fn unsubscribe(&self, id: usize);
}

struct DerivedSignalData<
    Derivation: Clone + PartialEq + 'static,
    Deps: Clone + 'static,
    F: Fn(Deps) -> Derivation + 'static,
> {
    cache: Option<Derivation>,
    deps: Deps,
    compute: F,
    listeners: Vec<Option<Rc<RefCell<dyn FnMut(Derivation)>>>>,
}

pub struct DerivedSignal<
    Derivation: Clone + PartialEq + 'static,
    Deps: Clone + 'static,
    F: Fn(Deps) -> Derivation + 'static,
> {
    data: Rc<RefCell<DerivedSignalData<Derivation, Deps, F>>>,
}

impl<T: Clone + PartialEq, Deps: Clone, F: Fn(Deps) -> T> Observable<T>
    for DerivedSignal<T, Deps, F>
{
    fn peek(&self) -> T {
        let mut data = self.data.borrow_mut();
        if let Some(val) = &data.cache {
            return val.clone();
        }
        data.cache = Some((data.compute)(data.deps.clone()));
        data.cache.clone().unwrap()
    }

    fn subscribe<OnChange: FnMut(T) + 'static>(&self, on_change: OnChange) -> usize {
        let mut data = self.data.borrow_mut();
        let chosen_index = data
            .listeners
            .iter()
            .enumerate()
            .find(|(_, v)| v.is_none())
            .map(|i| i.0);
        if let Some(i) = chosen_index {
            data.listeners[i] = Some(Rc::new(RefCell::new(on_change)));
        } else {
            data.listeners.push(Some(Rc::new(RefCell::new(on_change))));
        }
        data.listeners.len() - 1
    }

    fn unsubscribe(&self, id: usize) {
        //todo!()
    }
}

impl<Derivation: Clone + PartialEq, Deps: Clone, F: Fn(Deps) -> Derivation>
    DerivedSignalData<Derivation, Deps, F>
{
    pub fn maybe_recompute(&mut self) {
        // TODO: do we need to check deps?
        if self.listeners.is_empty() {
            self.cache = None;
            return;
        }
        let prev = self.cache.clone();
        self.cache = Some((self.compute)(self.deps.clone()));
        // TODO: have a way to skip this for large things?
        if prev == self.cache {
            return;
        }
        let new = self.cache.clone().unwrap();
        for i in 0..self.listeners.len() {
            if let Some(listener) = &self.listeners[i] {
                (listener.borrow_mut())(new.clone());
            }
        }
    }
}

struct ArbitraryIdStore<V> {
    data: Vec<Option<V>>,
}

impl<V> ArbitraryIdStore<V> {
    fn add(&mut self, value: V) -> usize {
        // TODO: is there a more performant way to fill old gaps? remember, index matters - maybe this should be a hashmap instead
        let chosen_index = self
            .data
            .iter()
            .enumerate()
            .rev()
            .find(|(_, v)| v.is_none())
            .map(|i| i.0);
        if let Some(i) = chosen_index {
            self.data[i] = Some(value);
            return i;
        } else {
            self.data.push(Some(value));
            return self.data.len() - 1;
        }
    }
    fn delete(&mut self, id: usize) {
        self.data[id] = None;
    }
    fn get(&self, id: usize) -> Option<&V> {
        if self.data.get(id).unwrap_or(&None).is_none() {
            None
        } else {
            Some(&self.data[id].as_ref().unwrap())
        }
    }
}
pub struct Constant<T: Clone> {
    value: T,
}

impl<T: Clone> Observable<T> for Constant<T> {
    fn peek(&self) -> T {
        self.value.clone()
    }

    fn subscribe<F: FnMut(T) + 'static>(&self, _: F) -> usize {
        return 0;
    }

    fn unsubscribe(&self, _: usize) {}
}

pub struct UIContext {
    elements: ArbitraryIdStore<Box<dyn UIElement>>,
    pub elements_requesting_redraw: Rc<RefCell<HashSet<usize>>>,
    font: font8x8::unicode::BasicFonts,
    screen_buffer: Vec<u8>,
}

impl UIContext {
    pub fn new(font: font8x8::unicode::BasicFonts) -> UIContext {
        UIContext {
            elements: ArbitraryIdStore {
                data: Vec::with_capacity(64),
            },
            elements_requesting_redraw: Rc::new(RefCell::new(HashSet::with_capacity(64))),
            font,
            screen_buffer: alloc::vec![0 as u8; (SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize) / 8],
        }
    }
    pub fn mount<El: UIElement + 'static>(&mut self, element: El) -> usize {
        let id = self.elements.add(Box::new(element));
        self.elements_requesting_redraw.borrow_mut().insert(id);
        let el = self.elements.get(id).unwrap();
        el.mount_to_context(self, id);
        id
    }
    pub fn get_screen_buffer(&self) -> &Vec<u8> {
        &self.screen_buffer
    }
    pub fn handle_draw_requests(&mut self) {
        let mut pixels_needing_redraw = alloc::vec![0 as u8; self.screen_buffer.len()];

        // TODO: should we skip this if only one element needs a redraw?
        for id in self.elements_requesting_redraw.borrow().iter() {
            let el = self.elements.get(*id).unwrap();
            let rect = el.get_bounding_rect();
            // TODO: handle screen limits
            for y in rect.y..=(rect.y + rect.height as i16) {
                for x in rect.x..=(rect.x + rect.width as i16) {
                    set_bit_buffer_pixel(&mut pixels_needing_redraw, x as u8, y as u8, 1);
                }
            }
        }

        for el in self.elements.data.iter().filter_map(|maybe_el| {
            if let Some(el) = maybe_el {
                Some(el)
            } else {
                None
            }
        }) {
            let source_rect = el.get_bounding_rect();
            let mut rect = source_rect;

            let mut curr_parent_id = el.get_parent_id();
            while let Some(parent_id) = curr_parent_id {
                let parent_el = self.elements.get(parent_id).unwrap();
                let parent_rect = parent_el.get_bounding_rect();
                rect.x += parent_rect.x;
                rect.y += parent_rect.y;
                curr_parent_id = parent_el.get_parent_id();
            }

            // TODO: This should be empty most of the time - do we know that empty vectors are free?
            let pixel_positions = (rect.y..=(rect.y + rect.height as i16))
                .flat_map(|y| {
                    let prd = &pixels_needing_redraw;
                    (rect.x..=(rect.x + rect.width as i16)).filter_map(move |x| {
                        if get_bit_buffer_pixel(&prd, x as u8, y as u8) {
                            Some(((x - rect.x) as u8, (y - rect.y) as u8))
                        } else {
                            None
                        }
                    })
                })
                .collect::<Vec<_>>();

            for (pixel, (x, y)) in el
                // we clone because rust is a deeply unserious language
                .get_pixels(self, Box::new(pixel_positions.clone().into_iter()))
                .zip(
                    pixel_positions
                        .into_iter()
                        .map(|(x, y)| (x as i16 + rect.x, y as i16 + rect.y)),
                )
            {
                let idx = (y as usize) * (SCREEN_WIDTH as usize) + (x as usize);
                let byte_idx = idx / 8;
                let bit_idx = 7 - (idx % 8);
                if byte_idx < self.screen_buffer.len() {
                    if pixel != 0 {
                        self.screen_buffer[byte_idx] |= 1 << bit_idx;
                    } else {
                        self.screen_buffer[byte_idx] &= !(1 << bit_idx);
                    }
                }
            }
        }

        self.elements_requesting_redraw.borrow_mut().clear();
    }
}

pub trait UIElement {
    fn mount_to_context(&self, ctx: &UIContext, id: usize);
    // Coordinates are in element space. width and height describes size of drawn region, not size of element
    fn get_pixels(
        &self,
        ctx: &UIContext,
        iterator: Box<dyn Iterator<Item = (u8, u8)>>,
    ) -> Box<dyn Iterator<Item = u8>>;
    fn get_bounding_rect(&self) -> BoundingRect;
    fn get_parent_id(&self) -> Option<usize>;
}

#[derive(Clone, Copy)]
pub struct BoundingRect {
    pub x: i16,
    pub y: i16,
    pub width: u8,
    pub height: u8,
}

pub struct TextUIElement<TextObservable: Observable<String>> {
    text: TextObservable,
    rect: BoundingRect,
    parent_id: usize,
}

impl<TO: Observable<String>> TextUIElement<TO> {
    pub fn new(text: TO, rect: BoundingRect, parent_id: usize) -> TextUIElement<TO> {
        TextUIElement {
            text,
            rect,
            parent_id,
        }
    }
}

impl<TO: Observable<String>> UIElement for TextUIElement<TO> {
    fn mount_to_context(&self, ctx: &UIContext, id: usize) {
        // TODO: Unsubscribe on unmount
        let els = ctx.elements_requesting_redraw.clone();
        let subscription_id = self.text.subscribe(move |_| {
            let mut ctx_borrowed = els.borrow_mut();
            ctx_borrowed.insert(id);
        });
    }
    fn get_pixels(
        &self,
        ctx: &UIContext,
        requested_pixels: Box<dyn Iterator<Item = (u8, u8)>>,
    ) -> Box<dyn Iterator<Item = u8>> {
        let text = self.text.peek();
        let font = &ctx.font;
        let char_width = 8 as usize;
        let char_height = 8 as usize;

        let text = text.clone();
        let iter = requested_pixels.map(|(x, y)| {
            let value = text.clone();
            if y < 0 || x < 0 || (y as usize) >= char_height {
                return 0;
            }
            let y = y as usize;
            let x = x as usize;

            let char_idx = x / char_width;
            let col = x % char_width;
            let row = y % char_height;

            let c_option = value.chars().nth(char_idx);
            if let Some(c) = c_option {
                let glyph = font.get(c).unwrap_or_default();
                let row_bits = glyph[row].reverse_bits();
                if (row_bits & (1 << (7 - col))) != 0 {
                    1
                } else {
                    0
                }
            } else {
                0
            }
        });

        Box::new(iter.collect::<Vec<_>>().into_iter())
    }
    fn get_bounding_rect(&self) -> BoundingRect {
        self.rect
    }
    fn get_parent_id(&self) -> Option<usize> {
        Some(self.parent_id)
    }
}

pub struct RectUIElement {
    parent_id: Option<usize>,
    rect: BoundingRect,
    color: u8,
}

impl RectUIElement {
    pub fn new(parent_id: Option<usize>, rect: BoundingRect, color: u8) -> RectUIElement {
        RectUIElement {
            rect,
            parent_id,
            color,
        }
    }
}

impl UIElement for RectUIElement {
    fn mount_to_context(&self, ctx: &UIContext, id: usize) {}
    fn get_pixels(
        &self,
        ctx: &UIContext,
        requested_pixels: Box<dyn Iterator<Item = (u8, u8)>>,
    ) -> Box<dyn Iterator<Item = u8>> {
        let color = self.color;
        Box::new(requested_pixels.map(move |_| color))
    }
    fn get_bounding_rect(&self) -> BoundingRect {
        self.rect
    }
    fn get_parent_id(&self) -> Option<usize> {
        self.parent_id
    }
}

fn set_bit_buffer_pixel(buffer: &mut [u8], x: u8, y: u8, color: u8) {
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

fn get_bit_buffer_pixel(buffer: &[u8], x: u8, y: u8) -> bool {
    if x >= SCREEN_WIDTH || y >= SCREEN_HEIGHT {
        return false;
    }
    let byte_index = y as usize * (SCREEN_WIDTH as usize / 8) + (x as usize / 8);
    let bit_index = 7 - (x % 8);

    if byte_index >= buffer.len() {
        return false;
    }

    (buffer[byte_index] >> bit_index) & 1 == 1
}
