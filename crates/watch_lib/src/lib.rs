#![no_std]
extern crate alloc;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use hashbrown::HashSet;

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
                listeners: Vec::new(),
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
            listeners: Vec::new(),
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
            listeners: Vec::new(),
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
}

impl UIContext {
    pub fn mount<El: UIElement + 'static>(&mut self, element: El) {
        let id = self.elements.add(Box::new(element));
        let el = self.elements.get(id);
        el.unwrap().draw_pixel(0, 0);
    }
}

pub trait UIElement {
    fn mount_to_context(&self, ctx: Rc<RefCell<UIContext>>, id: usize);
    fn draw_pixel(&self, x: u8, y: u8) -> u8;
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

impl<TO: Observable<String>> UIElement for TextUIElement<TO> {
    fn mount_to_context(&self, ctx: Rc<RefCell<UIContext>>, id: usize) {
        // TODO: Unsubscribe on unmount
        let subscription_id = self.text.subscribe(move |_| {
            let mut ctx_borrowed = ctx.borrow_mut();
            ctx_borrowed.elements_requesting_redraw.insert(id);
        });
    }
    fn draw_pixel(&self, x: u8, y: u8) -> u8 {
        0
    }
}
