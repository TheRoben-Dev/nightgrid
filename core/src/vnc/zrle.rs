//! ZRLE (RFC 6143 §7.7.6): zlib-compressed 64×64 tiles of raw, solid,
//! palette and run-length data. One zlib stream lasts the whole session.
//!
//! Pixels are "CPIXELs": with the 32-bit true-colour format Nightgrid asks
//! for (red in the low byte), each is 3 bytes R, G, B.

use flate2::{Decompress, FlushDecompress, Status};

use super::frame::{Frame, Rect};

const TILE: u16 = 64;

pub struct Zrle {
    z: Decompress,
}

impl Default for Zrle {
    fn default() -> Self {
        Zrle { z: Decompress::new(true) }
    }
}

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn u8(&mut self) -> Result<u8, String> {
        let v = *self.b.get(self.i).ok_or("ZRLE data ended early")?;
        self.i += 1;
        Ok(v)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.b.get(self.i..self.i + n).ok_or("ZRLE data ended early")?;
        self.i += n;
        Ok(s)
    }
    fn px(&mut self) -> Result<[u8; 3], String> {
        let s = self.take(3)?;
        Ok([s[0], s[1], s[2]])
    }
    fn run(&mut self) -> Result<usize, String> {
        let mut n = 1usize;
        loop {
            let b = self.u8()?;
            n += usize::from(b);
            if b != 255 {
                return Ok(n);
            }
        }
    }
}

impl Zrle {
    /// Inflates one rectangle's worth of compressed bytes.
    fn inflate(&mut self, data: &[u8]) -> Result<Vec<u8>, String> {
        let mut out = Vec::with_capacity(data.len() * 4 + 4096);
        let start = self.z.total_in();
        loop {
            let used = (self.z.total_in() - start) as usize;
            if out.capacity() - out.len() < 16 * 1024 {
                out.reserve(out.capacity().max(64 * 1024));
            }
            let room = out.capacity() - out.len();
            let before = (self.z.total_in(), self.z.total_out());
            let st = self
                .z
                .decompress_vec(&data[used..], &mut out, FlushDecompress::Sync)
                .map_err(|e| format!("ZRLE: {e}"))?;
            let used = (self.z.total_in() - start) as usize;
            let wrote = (self.z.total_out() - before.1) as usize;
            if st == Status::StreamEnd || (used >= data.len() && wrote < room) {
                return Ok(out);
            }
            if (self.z.total_in(), self.z.total_out()) == before {
                return Err("ZRLE: the stream stalled".into());
            }
        }
    }

    /// Decodes a ZRLE rectangle into `frame`.
    pub fn decode(&mut self, data: &[u8], r: Rect, frame: &mut Frame) -> Result<(), String> {
        let raw = self.inflate(data)?;
        let mut c = Cursor { b: &raw, i: 0 };
        let mut ty = r.y;
        while ty < r.y + r.h {
            let th = TILE.min(r.y + r.h - ty);
            let mut tx = r.x;
            while tx < r.x + r.w {
                let tw = TILE.min(r.x + r.w - tx);
                tile(&mut c, Rect { x: tx, y: ty, w: tw, h: th }, frame)?;
                tx += tw;
            }
            ty += th;
        }
        Ok(())
    }
}

fn tile(c: &mut Cursor<'_>, t: Rect, f: &mut Frame) -> Result<(), String> {
    let (x0, y0, w, h) = (usize::from(t.x), usize::from(t.y), usize::from(t.w), usize::from(t.h));
    let sub = c.u8()?;
    match sub {
        0 => {
            for y in 0..h {
                for x in 0..w {
                    let p = c.px()?;
                    f.put(x0 + x, y0 + y, p);
                }
            }
        }
        1 => {
            let p = c.px()?;
            f.fill(t, p);
        }
        2..=16 => {
            let n = usize::from(sub);
            let pal: Vec<[u8; 3]> = (0..n).map(|_| c.px()).collect::<Result<_, _>>()?;
            let bits = match n {
                2 => 1,
                3..=4 => 2,
                _ => 4,
            };
            let mask = (1u8 << bits) - 1;
            for y in 0..h {
                let mut byte = 0u8;
                let mut left = 0;
                for x in 0..w {
                    if left == 0 {
                        byte = c.u8()?;
                        left = 8;
                    }
                    left -= bits;
                    let idx = usize::from((byte >> left) & mask);
                    f.put(x0 + x, y0 + y, *pal.get(idx).ok_or("ZRLE palette index out of range")?);
                }
            }
        }
        128 | 130..=255 => {
            let pal: Vec<[u8; 3]> = if sub == 128 {
                Vec::new()
            } else {
                (0..usize::from(sub - 128)).map(|_| c.px()).collect::<Result<_, _>>()?
            };
            let total = w * h;
            let mut i = 0;
            while i < total {
                let (p, n) = if sub == 128 {
                    let p = c.px()?;
                    (p, c.run()?)
                } else {
                    let b = c.u8()?;
                    let p = *pal.get(usize::from(b & 127)).ok_or("ZRLE palette index out of range")?;
                    (p, if b & 128 != 0 { c.run()? } else { 1 })
                };
                if i + n > total {
                    return Err("ZRLE run past the tile".into());
                }
                for k in i..i + n {
                    f.put(x0 + k % w, y0 + k / w, p);
                }
                i += n;
            }
        }
        _ => return Err(format!("ZRLE: unknown tile type {sub}")),
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use flate2::{Compress, Compression, FlushCompress};

    /// Compresses tiles the way a server would (one stream, sync-flushed).
    pub(crate) fn pack(z: &mut Compress, raw: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(raw.len() + 64);
        let start = z.total_in();
        loop {
            out.reserve(4096);
            let used = (z.total_in() - start) as usize;
            z.compress_vec(&raw[used..], &mut out, FlushCompress::Sync).unwrap();
            if (z.total_in() - start) as usize >= raw.len() && out.capacity() > out.len() {
                break;
            }
        }
        out
    }

    fn px(f: &Frame, x: usize, y: usize) -> [u8; 3] {
        let o = (y * usize::from(f.width) + x) * 4;
        [f.pixels[o], f.pixels[o + 1], f.pixels[o + 2]]
    }

    #[test]
    fn decodes_every_tile_type_across_rects() {
        let mut z = Compress::new(Compression::default(), true);
        let mut d = Zrle::default();
        let mut f = Frame::new(70, 4);

        // Rect 1: 70×2 → two tiles (64 wide solid, 6 wide raw).
        let mut raw = vec![1, 9, 8, 7];
        raw.push(0);
        for i in 0..12u8 {
            raw.extend_from_slice(&[i, i, i]);
        }
        d.decode(&pack(&mut z, &raw), Rect { x: 0, y: 0, w: 70, h: 2 }, &mut f).unwrap();
        assert_eq!(px(&f, 0, 0), [9, 8, 7]);
        assert_eq!(px(&f, 63, 1), [9, 8, 7]);
        assert_eq!(px(&f, 64, 0), [0, 0, 0]);
        assert_eq!(px(&f, 69, 1), [11, 11, 11]);

        // Rect 2 (same zlib stream): packed palette, 2 colours, 3×1.
        let raw = vec![2, 1, 1, 1, 2, 2, 2, 0b1010_0000];
        d.decode(&pack(&mut z, &raw), Rect { x: 0, y: 2, w: 3, h: 1 }, &mut f).unwrap();
        assert_eq!([px(&f, 0, 2), px(&f, 1, 2), px(&f, 2, 2)], [[2, 2, 2], [1, 1, 1], [2, 2, 2]]);

        // Rect 3: plain RLE 4×1: run of 3 then 1.
        let raw = vec![128, 5, 5, 5, 2, 6, 6, 6, 0];
        d.decode(&pack(&mut z, &raw), Rect { x: 0, y: 3, w: 4, h: 1 }, &mut f).unwrap();
        assert_eq!(px(&f, 2, 3), [5, 5, 5]);
        assert_eq!(px(&f, 3, 3), [6, 6, 6]);

        // Rect 4: palette RLE, 2 colours, 4×1: index 1 run 2, then index 0 twice.
        let raw = vec![130, 3, 3, 3, 4, 4, 4, 0x81, 1, 0, 0];
        d.decode(&pack(&mut z, &raw), Rect { x: 0, y: 3, w: 4, h: 1 }, &mut f).unwrap();
        assert_eq!(
            [px(&f, 0, 3), px(&f, 1, 3), px(&f, 2, 3), px(&f, 3, 3)],
            [[4, 4, 4], [4, 4, 4], [3, 3, 3], [3, 3, 3]]
        );
    }

    #[test]
    fn rejects_bad_data() {
        let mut z = Compress::new(Compression::default(), true);
        let mut d = Zrle::default();
        let mut f = Frame::new(4, 1);
        let raw = vec![128, 1, 1, 1, 9]; // a run of 10 in a 4-pixel tile
        assert!(d.decode(&pack(&mut z, &raw), Rect { x: 0, y: 0, w: 4, h: 1 }, &mut f).is_err());
        let mut d = Zrle::default();
        assert!(d.decode(b"not zlib", Rect { x: 0, y: 0, w: 4, h: 1 }, &mut f).is_err());
    }
}
