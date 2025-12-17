#![no_std]
extern crate alloc;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::cmp::{max, min};
use font8x8::UnicodeFonts;
use hashbrown::HashSet;

// TODO: maybe signals becomes a split module
pub mod signals;
pub use signals::*;

pub const SCREEN_WIDTH: u8 = 200;
pub const SCREEN_HEIGHT: u8 = 200;

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

pub struct UIContext {
    elements: ArbitraryIdStore<Box<dyn UIElement>>,
    pub elements_requesting_redraw: Rc<RefCell<HashSet<usize>>>,
    font: font8x8::unicode::BasicFonts,
    screen_buffer: Vec<u8>,
    // Scratch buffers to avoid per-frame allocations
    scratch_redraw_sources: Vec<BoundingRect>,
    scratch_optimized_regions: Vec<BoundingRect>,
    scratch_sweep_normalized: Vec<BoundingRect>,
    scratch_sweep_x_edges: Vec<i16>,
    scratch_sweep_y_spans: Vec<(i16, i16)>,
    scratch_region_intersections: Vec<BoundingRect>,
}

impl UIContext {
    pub fn new(font: font8x8::unicode::BasicFonts) -> UIContext {
        UIContext {
            elements: ArbitraryIdStore {
                data: {
                    let mut vec: Vec<Option<Box<dyn UIElement>>> = Vec::with_capacity(64);
                    let el = RectUIElement::new(
                        0,
                        BoundingRect {
                            x: 0,
                            y: 0,
                            width: SCREEN_WIDTH,
                            height: SCREEN_HEIGHT,
                        },
                        // TODO: make invisible
                        0,
                    );
                    vec.push(Some(Box::new(el)));
                    vec
                },
            },
            elements_requesting_redraw: Rc::new(RefCell::new(HashSet::with_capacity(64))),
            font,
            screen_buffer: alloc::vec![0 as u8; (SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize) / 8],
            scratch_redraw_sources: Vec::with_capacity(64),
            scratch_optimized_regions: Vec::with_capacity(64),
            scratch_sweep_normalized: Vec::with_capacity(64),
            scratch_sweep_x_edges: Vec::with_capacity(128),
            scratch_sweep_y_spans: Vec::with_capacity(128),
            scratch_region_intersections: Vec::with_capacity(64),
        }
    }

    fn calculate_absolute_rect(&self, el_id: usize) -> BoundingRect {
        let el = self.elements.get(el_id).unwrap();
        let mut rect = el.get_bounding_rect();
        let mut curr_parent_id = el.get_parent_id();
        while curr_parent_id != 0 {
            let parent_el = self.elements.get(curr_parent_id).unwrap();
            let parent_rect = parent_el.get_bounding_rect();
            rect.x += parent_rect.x;
            rect.y += parent_rect.y;
            curr_parent_id = parent_el.get_parent_id();
        }
        rect
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
        let mut elements_requesting_redraw = self.elements_requesting_redraw.borrow_mut();
        // A quarter of the screen is the largest amount that can be partially updated - otherwise, we do a full update
        let partial_area_limit = (SCREEN_HEIGHT as usize * SCREEN_WIDTH as usize) / 4;
        let mut tracked_area: usize = 0;
        self.scratch_redraw_sources.clear();
        for id in elements_requesting_redraw.iter() {
            let rect = self.calculate_absolute_rect(*id);
            tracked_area =
                tracked_area.saturating_add((rect.width as usize) * (rect.height as usize));
            self.scratch_redraw_sources.push(rect);
        }

        let doing_full_redraw =
            elements_requesting_redraw.len() > 16 || tracked_area > partial_area_limit;

        self.scratch_optimized_regions.clear();
        if doing_full_redraw {
            self.scratch_optimized_regions.push(BoundingRect {
                x: 0,
                y: 0,
                width: SCREEN_WIDTH,
                height: SCREEN_HEIGHT,
            });
        } else {
            sweep_merge_rectangles(
                &self.scratch_redraw_sources,
                &mut self.scratch_optimized_regions,
                &mut self.scratch_sweep_normalized,
                &mut self.scratch_sweep_x_edges,
                &mut self.scratch_sweep_y_spans,
            );
        }

        for (id, maybe_el) in self.elements.data.iter().enumerate() {
            let Some(el) = maybe_el else {
                continue;
            };

            let rect = self.calculate_absolute_rect(id);
            let regions_iter: &[_] =
                if doing_full_redraw || elements_requesting_redraw.contains(&id) {
                    core::slice::from_ref(&rect)
                } else {
                    self.scratch_region_intersections.clear();
                    for region in self.scratch_optimized_regions.iter() {
                        if let Some(intersect) = region.intersection(&rect) {
                            self.scratch_region_intersections.push(intersect);
                        }
                    }
                    if self.scratch_region_intersections.is_empty() {
                        continue;
                    }
                    &self.scratch_region_intersections
                };

            for region in regions_iter {
                for y in region.y..(region.y + region.height as i16) {
                    for x in region.x..(region.x + region.width as i16) {
                        let idx = (y as usize) * (SCREEN_WIDTH as usize) + (x as usize);
                        let byte_idx = idx / 8;
                        let bit_idx = 7 - (idx % 8);
                        if byte_idx < self.screen_buffer.len() {
                            let pixel = el.get_pixel(
                                self,
                                (x as i16 - rect.x) as u8,
                                (y as i16 - rect.y) as u8,
                            );
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

        elements_requesting_redraw.clear();
    }
}

pub trait UIElement {
    fn mount_to_context(&self, ctx: &UIContext, id: usize);
    // Coordinates are in element space. width and height describes size of drawn region, not size of element
    fn get_pixel(&self, ctx: &UIContext, x: u8, y: u8) -> u8;
    fn get_bounding_rect(&self) -> BoundingRect;
    fn get_parent_id(&self) -> usize;
}

#[derive(Clone, Copy)]
pub struct BoundingRect {
    pub x: i16,
    pub y: i16,
    pub width: u8,
    pub height: u8,
}

impl BoundingRect {
    pub fn contains_point(&self, px: i16, py: i16) -> bool {
        px >= self.x
            && px < self.x + self.width as i16
            && py >= self.y
            && py < self.y + self.height as i16
    }
    pub fn overlaps(&self, other: &BoundingRect) -> bool {
        self.x < other.x + other.width as i16
            && self.x + self.width as i16 > other.x
            && self.y < other.y + other.height as i16
            && self.y + self.height as i16 > other.y
    }

    pub fn intersection(&self, other: &BoundingRect) -> Option<BoundingRect> {
        let x0 = core::cmp::max(self.x, other.x);
        let y0 = core::cmp::max(self.y, other.y);
        let x1 = core::cmp::min(self.x + self.width as i16, other.x + other.width as i16);
        let y1 = core::cmp::min(self.y + self.height as i16, other.y + other.height as i16);

        if x0 > x1 || y0 > y1 {
            None
        } else {
            Some(BoundingRect {
                x: x0,
                y: y0,
                width: (x1 - x0) as u8,
                height: (y1 - y0) as u8,
            })
        }
    }
}

fn normalize_rect_to_screen(rect: &BoundingRect) -> Option<(i16, i16, i16, i16)> {
    let x0 = max(0, rect.x);
    let y0 = max(0, rect.y);
    let x1 = min(SCREEN_WIDTH as i16, rect.x + rect.width as i16);
    let y1 = min(SCREEN_HEIGHT as i16, rect.y + rect.height as i16);

    if x0 >= x1 || y0 >= y1 {
        None
    } else {
        Some((x0, x1, y0, y1))
    }
}

fn sweep_merge_rectangles(
    rects: &[BoundingRect],
    out: &mut Vec<BoundingRect>,
    normalized: &mut Vec<BoundingRect>,
    x_edges: &mut Vec<i16>,
    y_spans: &mut Vec<(i16, i16)>,
) {
    out.clear();
    normalized.clear();
    x_edges.clear();

    for rect in rects {
        if let Some((x0, x1, y0, y1)) = normalize_rect_to_screen(rect) {
            normalized.push(BoundingRect {
                x: x0,
                y: y0,
                width: (x1 - x0) as u8,
                height: (y1 - y0) as u8,
            });
            x_edges.push(x0);
            x_edges.push(x1);
        }
    }

    if normalized.is_empty() {
        return;
    }

    x_edges.sort_unstable();
    x_edges.dedup();

    out.clear();
    y_spans.clear();

    for pair in x_edges.windows(2) {
        let x_start = pair[0];
        let x_end = pair[1];
        if x_start >= x_end {
            continue;
        }

        y_spans.clear();
        for rect in normalized.iter() {
            if rect.x <= x_start && rect.x + rect.width as i16 >= x_end {
                y_spans.push((rect.y, rect.y + rect.height as i16));
            }
        }

        if y_spans.is_empty() {
            continue;
        }

        y_spans.sort_by_key(|(start, _)| *start);

        let mut current_span = y_spans[0];
        for span in y_spans.iter().skip(1) {
            if span.0 <= current_span.1 {
                current_span.1 = max(current_span.1, span.1);
            } else {
                out.push(BoundingRect {
                    x: x_start,
                    y: current_span.0,
                    width: (x_end - x_start) as u8,
                    height: (current_span.1 - current_span.0) as u8,
                });
                current_span = *span;
            }
        }

        out.push(BoundingRect {
            x: x_start,
            y: current_span.0,
            width: (x_end - x_start) as u8,
            height: (current_span.1 - current_span.0) as u8,
        });
    }
}

pub struct TextUIElement<TextObservable: Observable<String>> {
    text: TextObservable,
    rect: BoundingRect,
    parent_id: usize,
}

impl<TO: Observable<String>> TextUIElement<TO> {
    pub fn new(text: &TO, rect: BoundingRect, parent_id: usize) -> TextUIElement<TO> {
        TextUIElement {
            text: text.clone(),
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
    fn get_pixel(&self, ctx: &UIContext, x: u8, y: u8) -> u8 {
        let text = &self.text.peek();
        let font = &ctx.font;
        let char_width = 8 as usize;
        let char_height = 8 as usize;

        let y = y as usize;
        let x = x as usize;
        if y >= char_height {
            return 0;
        }

        let char_idx = x / char_width;
        let col = x % char_width;
        let row = y % char_height;

        let c_option = text.chars().nth(char_idx);
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
    }
    fn get_bounding_rect(&self) -> BoundingRect {
        self.rect
    }
    fn get_parent_id(&self) -> usize {
        self.parent_id
    }
}

pub struct RectUIElement {
    parent_id: usize,
    rect: BoundingRect,
    color: u8,
}

impl RectUIElement {
    pub fn new(parent_id: usize, rect: BoundingRect, color: u8) -> RectUIElement {
        RectUIElement {
            rect,
            parent_id,
            color,
        }
    }
}

impl UIElement for RectUIElement {
    fn mount_to_context(&self, _ctx: &UIContext, _id: usize) {}
    fn get_pixel(&self, _ctx: &UIContext, _x: u8, _y: u8) -> u8 {
        self.color
    }
    fn get_bounding_rect(&self) -> BoundingRect {
        self.rect
    }
    fn get_parent_id(&self) -> usize {
        self.parent_id
    }
}
