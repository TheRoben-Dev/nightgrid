//! Drops `<think>…</think>` blocks from a streamed reply. Hybrid models
//! (Qwen3 8B) are asked not to think, but still open with an empty block,
//! and a tag can arrive split across pieces.

const OPEN: &str = "<think>";
const CLOSE: &str = "</think>";

#[derive(Debug, Default)]
pub struct ThinkFilter {
    inside: bool,
    /// Text held back because it may be the start of a tag.
    held: String,
    /// Nothing shown yet: leading whitespace after a block is dropped too.
    fresh: bool,
}

/// Length of the longest suffix of `s` that is a prefix of `tag`.
fn partial(s: &str, tag: &str) -> usize {
    (1..tag.len().min(s.len() + 1))
        .rev()
        .find(|&n| s.is_char_boundary(s.len() - n) && tag.starts_with(&s[s.len() - n..]))
        .unwrap_or(0)
}

impl ThinkFilter {
    pub fn new() -> Self {
        Self { fresh: true, ..Default::default() }
    }

    /// Feeds a piece; returns what can be shown now.
    pub fn push(&mut self, piece: &str) -> String {
        self.held.push_str(piece);
        let mut out = String::new();
        loop {
            let tag = if self.inside { CLOSE } else { OPEN };
            if let Some(i) = self.held.find(tag) {
                if !self.inside {
                    out.push_str(&self.held[..i]);
                }
                self.held.drain(..i + tag.len());
                self.inside = !self.inside;
                continue;
            }
            let keep = partial(&self.held, tag);
            let cut = self.held.len() - keep;
            if !self.inside {
                out.push_str(&self.held[..cut]);
            }
            self.held.drain(..cut);
            break;
        }
        if self.fresh {
            let t = out.trim_start();
            if t.is_empty() {
                return String::new();
            }
            self.fresh = false;
            return t.to_owned();
        }
        out
    }

    /// The rest at the end (an unfinished tag that wasn't one).
    pub fn finish(&mut self) -> String {
        let rest = std::mem::take(&mut self.held);
        if self.inside {
            String::new()
        } else {
            rest
        }
    }
}

/// The whole reply without think blocks.
pub fn strip(text: &str) -> String {
    let mut f = ThinkFilter::new();
    let mut s = f.push(text);
    s.push_str(&f.finish());
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_blocks_across_pieces() {
        let mut f = ThinkFilter::new();
        let mut out = String::new();
        for p in ["<th", "ink>\n", "hmm</thi", "nk>\n\nThe con", "tainer is <b>fine</b>.", " a<", "t"] {
            out.push_str(&f.push(p));
        }
        out.push_str(&f.finish());
        assert_eq!(out, "The container is <b>fine</b>. a<t");
        assert_eq!(strip("<think>\n\n</think>\n\nHello"), "Hello");
        assert_eq!(strip("No tags here"), "No tags here");
        assert_eq!(strip("ok <think>secret"), "ok ");
    }
}
