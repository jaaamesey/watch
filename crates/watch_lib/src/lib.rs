#![no_std]
extern crate alloc;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use hashbrown::HashSet;

pub const SCREEN_WIDTH: u8 = 200;
pub const SCREEN_HEIGHT: u8 = 200;

// TODO: safely handle unwraps
struct SignalData<T: Clone> {
    value: T,
    listeners: Vec<Option<Rc<RefCell<dyn FnMut(T)>>>>,
}

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
    D0: Clone + Copy + 'static,
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
    D0: Clone + Copy + 'static,
    D1: Clone + Copy + 'static,
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
    Deps: Clone + Copy + 'static,
    F: Fn(Deps) -> Derivation + 'static,
> {
    cache: Option<Derivation>,
    deps: Deps,
    compute: F,
    listeners: Vec<Option<Rc<RefCell<dyn FnMut(Derivation)>>>>,
}

pub struct DerivedSignal<
    Derivation: Clone + PartialEq + 'static,
    Deps: Clone + Copy + 'static,
    F: Fn(Deps) -> Derivation + 'static,
> {
    data: Rc<RefCell<DerivedSignalData<Derivation, Deps, F>>>,
}

impl<T: Clone + PartialEq, Deps: Clone + Copy, F: Fn(Deps) -> T> Observable<T>
    for DerivedSignal<T, Deps, F>
{
    fn peek(&self) -> T {
        let mut data = self.data.borrow_mut();
        if let Some(val) = &data.cache {
            return val.clone();
        }
        data.cache = Some((data.compute)(data.deps));
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

impl<Derivation: Clone + PartialEq, Deps: Clone + Copy, F: Fn(Deps) -> Derivation>
    DerivedSignalData<Derivation, Deps, F>
{
    pub fn maybe_recompute(&mut self) {
        // TODO: do we need to check deps?
        if self.listeners.is_empty() {
            self.cache = None;
            return;
        }
        let prev = self.cache.clone();
        self.cache = Some((self.compute)(self.deps));
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
    elements_requesting_redraw: HashSet<usize>,
    font: font8x8::unicode::BasicFonts,
    screen_buffer: Vec<u8>,
}

impl UIContext {
    pub fn new(font: font8x8::unicode::BasicFonts) -> UIContext {
        UIContext {
            elements: ArbitraryIdStore {
                data: Vec::with_capacity(64),
            },
            elements_requesting_redraw: HashSet::with_capacity(64),
            font,
            screen_buffer: alloc::vec![0 as u8; (SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize) / 8],
        }
    }
    pub fn mount<El: UIElement + 'static>(&mut self, element: El) {
        let id = self.elements.add(Box::new(element));
        // let el = self.elements.get(id);
        // let rect = el.unwrap().get_bounding_rect();
        self.elements_requesting_redraw.insert(id);
        // el.unwrap()._pixel(self, 0, 0);
    }
    pub fn get_screen_buffer(&self) -> &Vec<u8> {
        &self.screen_buffer
    }
    pub fn handle_draw_requests(&mut self) {
        for id in self.elements_requesting_redraw.iter() {
            let el = self.elements.get(*id).unwrap();
            // TODO: redraw els behind (if not already in this list - actually maybe we need a sorting strategy)
            // (or maybe elements requesting redraw just determines a bounding box to redraw??? idk)
            let rect = el.get_bounding_rect();
            let mut pixel_iter = el.get_pixels(self, rect);
            for y in rect.y..=(rect.y + rect.height) {
                for x in rect.x..=(rect.x + rect.width) {
                    if let Some(pixel) = pixel_iter.next() {
                        let screen_width = SCREEN_WIDTH as usize;
                        let idx = (y as usize) * screen_width + (x as usize);
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
            }
        }
    }
}

pub trait UIElement {
    fn mount_to_context(&self, ctx: Rc<RefCell<UIContext>>, id: usize);
    // Coordinates are in element space
    fn get_pixels(&self, ctx: &UIContext, rect: BoundingRect) -> Box<dyn Iterator<Item = u8>>;
    fn get_bounding_rect(&self) -> BoundingRect;
}

pub struct TextUIElement<TextObservable: Observable<String>> {
    text: TextObservable,
    pos: (u8, u8),
}

impl<TO: Observable<String>> TextUIElement<TO> {
    pub fn new(text: TO, pos: (u8, u8)) -> TextUIElement<TO> {
        TextUIElement { text, pos }
    }
}

#[derive(Clone, Copy)]
struct BoundingRect {
    x: u8,
    y: u8,
    width: u8,
    height: u8,
}

impl<TO: Observable<String>> UIElement for TextUIElement<TO> {
    fn mount_to_context(&self, ctx: Rc<RefCell<UIContext>>, id: usize) {
        // TODO: Unsubscribe on unmount
        let subscription_id = self.text.subscribe(move |_| {
            let mut ctx_borrowed = ctx.borrow_mut();
            ctx_borrowed.elements_requesting_redraw.insert(id);
        });
    }
    fn get_pixels(&self, ctx: &UIContext, rect: BoundingRect) -> Box<dyn Iterator<Item = u8>> {
        let toggled = self.text.peek() == "on";
        Box::new((rect.y..=(rect.y + rect.height)).flat_map(move |y| {
            (rect.x..=(rect.x + rect.width)).map(move |x| if toggled && x % 2 == 0 { 1 } else { 0 })
        }))
    }
    fn get_bounding_rect(&self) -> BoundingRect {
        BoundingRect {
            x: 100,
            y: 100,
            width: 64,
            height: 16,
        }
    }
}
