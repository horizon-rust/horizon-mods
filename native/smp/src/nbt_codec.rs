//! Network NBT, for the stacks the auction house keeps in `economy.json` (`ItemStack.CODEC`'s tag, in hex): the
//! same bytes the game's own writer made, read and written here because the mod API hands over tags, not bytes.

use mod_api::{Compound, Tag};

const MAX_DEPTH: usize = 64;

fn utf(out: &mut Vec<u8>, units: &[u16]) -> bool {
    let mut bytes = Vec::with_capacity(units.len());
    for &u in units {
        match u {
            0x0001..=0x007F => bytes.push(u as u8),
            0x0000 | 0x0080..=0x07FF => {
                bytes.extend([0xC0 | (u >> 6) as u8, 0x80 | (u & 0x3F) as u8])
            }
            _ => bytes.extend([
                0xE0 | (u >> 12) as u8,
                0x80 | ((u >> 6) & 0x3F) as u8,
                0x80 | (u & 0x3F) as u8,
            ]),
        }
    }
    let Ok(len) = u16::try_from(bytes.len()) else {
        return false;
    };
    out.extend(len.to_be_bytes());
    out.extend(bytes);
    true
}

fn write_payload(tag: &Tag, out: &mut Vec<u8>, depth: usize) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    match tag {
        Tag::End => {}
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend(v.to_be_bytes()),
        Tag::Int(v) => out.extend(v.to_be_bytes()),
        Tag::Long(v) => out.extend(v.to_be_bytes()),
        Tag::Float(v) => out.extend(v.to_be_bytes()),
        Tag::Double(v) => out.extend(v.to_be_bytes()),
        Tag::ByteArray(v) => {
            out.extend((v.len() as i32).to_be_bytes());
            out.extend(v);
        }
        Tag::String(s) => {
            if !utf(out, &s.0) {
                return false;
            }
        }
        Tag::List(items) => {
            out.push(items.first().map_or(0, Tag::id));
            out.extend((items.len() as i32).to_be_bytes());
            for t in items {
                if t.id() != items[0].id() || !write_payload(t, out, depth + 1) {
                    return false;
                }
            }
        }
        Tag::Compound(map) => {
            for (k, v) in map {
                out.push(v.id());
                if !utf(out, &k.0) || !write_payload(v, out, depth + 1) {
                    return false;
                }
            }
            out.push(0);
        }
        Tag::IntArray(v) => {
            out.extend((v.len() as i32).to_be_bytes());
            for i in v {
                out.extend(i.to_be_bytes());
            }
        }
        Tag::LongArray(v) => {
            out.extend((v.len() as i32).to_be_bytes());
            for i in v {
                out.extend(i.to_be_bytes());
            }
        }
    }
    true
}

/// `NbtIo.writeAnyTag` (a nameless root).
pub fn write_network(tag: &Tag) -> Option<Vec<u8>> {
    let mut out = vec![tag.id()];
    write_payload(tag, &mut out, 0).then_some(out)
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let (head, rest) = self.0.split_at_checked(n)?;
        self.0 = rest;
        Some(head)
    }
    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }
    fn len(&mut self) -> Option<usize> {
        usize::try_from(i32::from_be_bytes(self.array()?))
            .ok()
            .filter(|&n| n <= self.0.len().max(1) * 8)
    }
    fn string(&mut self) -> Option<Vec<u16>> {
        let n = usize::from(u16::from_be_bytes(self.array()?));
        let bytes = self.take(n)?;
        let mut units = Vec::with_capacity(n);
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            match b {
                0x00..=0x7F => {
                    units.push(u16::from(b));
                    i += 1;
                }
                0xC0..=0xDF => {
                    units.push(u16::from(b & 0x1F) << 6 | u16::from(bytes.get(i + 1)? & 0x3F));
                    i += 2;
                }
                0xE0..=0xEF => {
                    units.push(
                        u16::from(b & 0x0F) << 12
                            | u16::from(bytes.get(i + 1)? & 0x3F) << 6
                            | u16::from(bytes.get(i + 2)? & 0x3F),
                    );
                    i += 3;
                }
                _ => return None,
            }
        }
        Some(units)
    }
    fn payload(&mut self, id: u8, depth: usize) -> Option<Tag> {
        if depth > MAX_DEPTH {
            return None;
        }
        Some(match id {
            1 => Tag::Byte(i8::from_be_bytes(self.array()?)),
            2 => Tag::Short(i16::from_be_bytes(self.array()?)),
            3 => Tag::Int(i32::from_be_bytes(self.array()?)),
            4 => Tag::Long(i64::from_be_bytes(self.array()?)),
            5 => Tag::Float(u32::from_be_bytes(self.array()?)),
            6 => Tag::Double(u64::from_be_bytes(self.array()?)),
            7 => {
                let n = self.len()?;
                Tag::ByteArray(self.take(n)?.to_vec())
            }
            8 => Tag::String(String::from_utf16_lossy(&self.string()?).as_str().into()),
            9 => {
                let [kind] = self.array()?;
                let n = self.len()?;
                if kind == 0 && n > 0 {
                    return None;
                }
                let mut items = Vec::new();
                for _ in 0..n {
                    items.push(self.payload(kind, depth + 1)?);
                }
                Tag::List(items)
            }
            10 => {
                let mut map = Compound::new();
                loop {
                    let [kind] = self.array()?;
                    if kind == 0 {
                        break;
                    }
                    let name = self.string()?;
                    let value = self.payload(kind, depth + 1)?;
                    map.insert(String::from_utf16_lossy(&name).as_str().into(), value);
                }
                Tag::Compound(map)
            }
            11 => {
                let n = self.len()?;
                let mut v = Vec::new();
                for _ in 0..n {
                    v.push(i32::from_be_bytes(self.array()?));
                }
                Tag::IntArray(v)
            }
            12 => {
                let n = self.len()?;
                let mut v = Vec::new();
                for _ in 0..n {
                    v.push(i64::from_be_bytes(self.array()?));
                }
                Tag::LongArray(v)
            }
            _ => return None,
        })
    }
}

/// `NbtIo.readAnyTag`: the root and what it covers, None for bytes that aren't a tag or leave some over.
pub fn read_network(bytes: &[u8]) -> Option<Tag> {
    let mut r = Reader(bytes);
    let [id] = r.array()?;
    let tag = r.payload(id, 0)?;
    r.0.is_empty().then_some(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stack_tag_reads_back_as_written_and_in_the_games_bytes() {
        let mut components = Compound::new();
        components.insert(
            "minecraft:custom_name".into(),
            Tag::String("Ünï \u{0}cødé ✓".into()),
        );
        components.insert(
            "minecraft:lore".into(),
            Tag::List(vec![Tag::Int(1), Tag::Int(2)]),
        );
        let mut stack = Compound::new();
        stack.insert("id".into(), Tag::String("minecraft:stone".into()));
        stack.insert("count".into(), Tag::Int(3));
        stack.insert("components".into(), Tag::Compound(components));
        let tag = Tag::Compound(stack);
        let bytes = write_network(&tag).expect("writes");
        assert_eq!(read_network(&bytes), Some(tag));
        // A compound root, `id` then `count` (names sorted), as Java's `NbtIo.writeAnyTag` writes it.
        let java: &[u8] = &[
            10, 3, 0, 5, b'c', b'o', b'u', b'n', b't', 0, 0, 0, 7, 8, 0, 2, b'i', b'd', 0, 3, b'a',
            b'b', b'c', 0,
        ];
        let mut want = Compound::new();
        want.insert("count".into(), Tag::Int(7));
        want.insert("id".into(), Tag::String("abc".into()));
        assert_eq!(read_network(java), Some(Tag::Compound(want)));
        assert_eq!(
            write_network(&read_network(java).unwrap()).as_deref(),
            Some(java)
        );
    }

    #[test]
    fn bytes_that_are_not_a_tag_are_refused() {
        assert_eq!(read_network(&[]), None);
        assert_eq!(read_network(&[10, 3, 0, 1]), None);
        assert_eq!(read_network(&[10, 0, 99]), None);
    }
}
