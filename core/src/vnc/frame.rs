//! The guest's screen as an RGBA buffer, plus the parts that changed since the
//! UI last fetched it.

/// A rectangle in framebuffer pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    fn area(self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }

    fn union(self, o: Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        let r = (self.x + self.w).max(o.x + o.w);
        let b = (self.y + self.h).max(o.y + o.h);
        Rect { x, y, w: r - x, h: b - y }
    }

    fn contains(self, o: Rect) -> bool {
        o.x >= self.x && o.y >= self.y && o.x + o.w <= self.x + self.w && o.y + o.h <= self.y + self.h
    }
}

/// Above this many separate dirty rectangles they're merged into one.
const MAX_RECTS: usize = 24;
/// The largest screen accepted (a guest asking for more is refused).
pub const MAX_SIDE: u16 = 8192;

#[derive(Debug, Default)]
pub struct Frame {
    pub width: u16,
    pub height: u16,
    /// RGBA, row-major, `width * height * 4` bytes.
    pub pixels: Vec<u8>,
    dirty: Vec<Rect>,
    /// The size changed: the next patch carries the whole screen.
    resized: bool,
}

impl Frame {
    pub fn new(width: u16, height: u16) -> Self {
        let mut f = Frame::default();
        f.resize(width, height);
        f
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.pixels = vec![0; usize::from(width) * usize::from(height) * 4];
        for px in self.pixels.chunks_exact_mut(4) {
            px[3] = 255;
        }
        self.dirty.clear();
        self.resized = true;
    }

    /// Whether `r` lies inside the screen.
    pub fn fits(&self, r: Rect) -> bool {
        u32::from(r.x) + u32::from(r.w) <= u32::from(self.width)
            && u32::from(r.y) + u32::from(r.h) <= u32::from(self.height)
    }

    pub fn mark(&mut self, r: Rect) {
        if r.w == 0 || r.h == 0 || self.resized {
            return;
        }
        if self.dirty.iter().any(|d| d.contains(r)) {
            return;
        }
        self.dirty.retain(|d| !r.contains(*d));
        self.dirty.push(r);
        if self.dirty.len() > MAX_RECTS {
            let all = self.dirty.iter().copied().reduce(Rect::union).unwrap_or(r);
            self.dirty = vec![all];
        }
    }

    pub fn has_changes(&self) -> bool {
        self.resized || !self.dirty.is_empty()
    }

    #[inline]
    fn offset(&self, x: usize, y: usize) -> usize {
        (y * usize::from(self.width) + x) * 4
    }

    /// Writes one pixel from R, G, B.
    #[inline]
    pub fn put(&mut self, x: usize, y: usize, rgb: [u8; 3]) {
        let o = self.offset(x, y);
        self.pixels[o..o + 3].copy_from_slice(&rgb);
        self.pixels[o + 3] = 255;
    }

    /// Fills a rectangle with one colour.
    pub fn fill(&mut self, r: Rect, rgb: [u8; 3]) {
        let px = [rgb[0], rgb[1], rgb[2], 255];
        for y in r.y..r.y + r.h {
            let o = self.offset(r.x.into(), y.into());
            for chunk in self.pixels[o..o + usize::from(r.w) * 4].chunks_exact_mut(4) {
                chunk.copy_from_slice(&px);
            }
        }
    }

    /// Copies raw 32-bit pixels laid out R, G, B, X (the format Nightgrid asks for).
    pub fn blit_rgbx(&mut self, r: Rect, data: &[u8]) {
        let row = usize::from(r.w) * 4;
        for (i, y) in (r.y..r.y + r.h).enumerate() {
            let o = self.offset(r.x.into(), y.into());
            let dst = &mut self.pixels[o..o + row];
            dst.copy_from_slice(&data[i * row..(i + 1) * row]);
            for px in dst.chunks_exact_mut(4) {
                px[3] = 255;
            }
        }
    }

    /// CopyRect: moves `r`'s size of pixels from (`sx`, `sy`) to `r`.
    pub fn copy(&mut self, r: Rect, sx: u16, sy: u16) {
        let row = usize::from(r.w) * 4;
        let rows: Vec<u16> = if sy < r.y { (0..r.h).rev().collect() } else { (0..r.h).collect() };
        for i in rows {
            let from = self.offset(sx.into(), usize::from(sy + i));
            let to = self.offset(r.x.into(), usize::from(r.y + i));
            self.pixels.copy_within(from..from + row, to);
        }
    }

    /// The changes since the last call, as one binary patch, or `None`.
    ///
    /// Layout (little-endian): `u8 full`, `u8 0`, `u16 width`, `u16 height`,
    /// `u16 count`, then per rectangle `u16 x, y, w, h` and `w*h*4` RGBA bytes.
    pub fn take_patch(&mut self) -> Option<Vec<u8>> {
        if !self.has_changes() {
            return None;
        }
        let whole = Rect { x: 0, y: 0, w: self.width, h: self.height };
        let area: u64 = self.dirty.iter().map(|r| r.area()).sum();
        let full = self.resized || area * 10 > whole.area() * 6;
        let rects = if full { vec![whole] } else { std::mem::take(&mut self.dirty) };
        self.dirty.clear();
        self.resized = false;
        let bytes: usize = rects.iter().map(|r| 8 + r.area() as usize * 4).sum();
        let mut out = Vec::with_capacity(8 + bytes);
        out.push(u8::from(full));
        out.push(0);
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&(rects.len() as u16).to_le_bytes());
        for r in rects {
            for v in [r.x, r.y, r.w, r.h] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            let row = usize::from(r.w) * 4;
            for y in r.y..r.y + r.h {
                let o = self.offset(r.x.into(), y.into());
                out.extend_from_slice(&self.pixels[o..o + row]);
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_carry_only_what_changed() {
        let mut f = Frame::new(100, 100);
        let first = f.take_patch().unwrap();
        assert_eq!(first[0], 1, "a new screen is sent whole");
        assert_eq!(first.len(), 8 + 8 + 100 * 100 * 4);
        assert!(f.take_patch().is_none());

        let r = Rect { x: 10, y: 20, w: 2, h: 3 };
        f.fill(r, [1, 2, 3]);
        f.mark(r);
        f.mark(Rect { x: 10, y: 20, w: 1, h: 1 }); // inside: ignored
        let p = f.take_patch().unwrap();
        assert_eq!(p[0], 0);
        assert_eq!(u16::from_le_bytes([p[6], p[7]]), 1);
        assert_eq!(&p[8..16], &[10, 0, 20, 0, 2, 0, 3, 0]);
        assert_eq!(&p[16..20], &[1, 2, 3, 255]);
        assert_eq!(p.len(), 16 + 2 * 3 * 4);
    }

    #[test]
    fn many_rects_merge_and_big_changes_go_whole() {
        let mut f = Frame::new(1000, 1000);
        f.take_patch();
        for i in 0..25u16 {
            f.mark(Rect { x: i * 10, y: i * 10, w: 5, h: 5 });
        }
        assert_eq!(f.dirty.len(), 1);
        assert_eq!(f.dirty[0], Rect { x: 0, y: 0, w: 245, h: 245 });
        f.mark(Rect { x: 0, y: 0, w: 1000, h: 900 });
        assert_eq!(f.take_patch().unwrap()[0], 1);
    }

    #[test]
    fn copy_handles_overlap() {
        let mut f = Frame::new(4, 4);
        for y in 0..4 {
            f.put(0, y, [y as u8, 0, 0]);
        }
        // Shift column 0 down by one row (overlapping source and destination).
        f.copy(Rect { x: 0, y: 1, w: 1, h: 3 }, 0, 0);
        let col: Vec<u8> = (0..4).map(|y| f.pixels[f.offset(0, y)]).collect();
        assert_eq!(col, vec![0, 0, 1, 2]);
        assert!(f.fits(Rect { x: 0, y: 0, w: 4, h: 4 }));
        assert!(!f.fits(Rect { x: 3, y: 0, w: 2, h: 1 }));
    }
}
