// show-me-what-you-got ported to Horizon as a native Rust mod.
//
// Upstream: https://github.com/apace100/show-me-what-you-got at commit
// 0ee8852 (2023-11-18), MIT (c) 2022 apace100. 1:1 port of the mod core:
// the SmwygItemMatch model, the per-sender MessageItemList (take-on-read,
// cleared on server start), the [[smwyg:...]] tag regex + SNBT extraction and
// chat decoration, the share-item packet (varint start/end + compound NBT)
// with the server receiver's deferred add, the shared chat text-field
// insertion state machine (cursor/selection/erase/write clamping math from
// TextFieldWidgetMixin, suggestion-offset tracking, stale-link handling), the
// chat screen send flow (tagged history entry, packet send, no history
// duplicate), and the client tick share gate (left-ctrl + chat key opens the
// chat with the focused slot's stack, latch semantics 1:1).
//
// Game-side seams (documented in README.md): tooltip composition in
// DrawContext.drawHoverEvent/drawTooltip with custom TooltipComponents
// (ItemStackTooltipComponent renders the hovered stack beside the tooltip,
// honoring the smwyg:hide_stack NBT flag; HorizontalLayoutTooltipComponent is
// the generic horizontal stacker), chat suggestion completion callback
// (ChatInputSuggestor.SuggestionWindow.complete), focused-slot and cursor
// stack access on container screens, per-key input polling (GLFW left ctrl +
// bound chat key), and chat HUD history writes.

use std::collections::HashMap;

pub const MOD_ID: &str = "smwyg";
pub const PACKET_ID: &str = "smwyg:share_item";
pub const HIDE_STACK_NBT: &str = "smwyg:hide_stack";
// en_us.json: "smwyg.chat.stale_link"
pub const STALE_LINK_TEXT: &str = "[stale link]";
pub const TAG_OPEN: &str = "[[smwyg:";
pub const TAG_CLOSE: &str = "]]";

// ---------------------------------------------------------------------------
// NBT (compound subset of Minecraft NBT, matching what this mod exchanges)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Default)]
pub enum NbtValue {
    #[default]
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(Vec<NbtValue>),
    Compound(NbtCompound),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct NbtCompound {
    entries: Vec<(String, NbtValue)>,
}

impl NbtCompound {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put(&mut self, key: impl Into<String>, value: NbtValue) {
        let key = key.into();
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    pub fn get(&self, key: &str) -> Option<&NbtValue> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(NbtValue::String(s)) => Some(s),
            _ => None,
        }
    }

    // Vanilla NbtCompound.getBoolean: only a Byte tag counts, any nonzero is true.
    pub fn get_boolean(&self, key: &str) -> bool {
        matches!(self.get(key), Some(NbtValue::Byte(b)) if *b != 0)
    }

    pub fn get_int(&self, key: &str) -> Option<i32> {
        match self.get(key) {
            Some(NbtValue::Int(i)) => Some(*i),
            _ => None,
        }
    }

    pub fn get_compound(&self, key: &str) -> Option<&NbtCompound> {
        match self.get(key) {
            Some(NbtValue::Compound(c)) => Some(c),
            _ => None,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &NbtValue)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    // --- SNBT string form (StringNbtReader subset, compound level) ---

    pub fn to_snbt(&self) -> String {
        let mut out = String::from("{");
        for (i, (k, v)) in self.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            write_snbt_key(&mut out, k);
            out.push(':');
            write_snbt_value(&mut out, v);
        }
        out.push('}');
        out
    }

    // Parses a compound from the start of `s` the way
    // StringNbtReader.parseCompound does: no EOF requirement, trailing input
    // after the closing brace is ignored. Returns Err when the leading value
    // is not a well-formed compound.
    pub fn parse_snbt(s: &str) -> Result<NbtCompound, ()> {
        let mut p = SnbtParser {
            s: s.as_bytes(),
            i: 0,
        };
        p.skip_ws();
        p.parse_compound()
    }
}

fn write_snbt_key(out: &mut String, key: &str) {
    if key.is_empty()
        || key.bytes().any(|b| {
            b == b'"'
                || b == b'\''
                || b == b'{'
                || b == b'}'
                || b == b'['
                || b == b']'
                || b == b','
                || b == b':'
                || b <= b' '
        })
    {
        write_quoted(out, key, '"');
    } else {
        out.push_str(key);
    }
}

fn write_quoted(out: &mut String, s: &str, quote: char) {
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ if c == quote => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\u{000c}' => out.push_str("\\f"),
            '\u{0008}' => out.push_str("\\b"),
            _ => out.push(c),
        }
    }
    out.push(quote);
}

fn write_snbt_value(out: &mut String, v: &NbtValue) {
    match v {
        NbtValue::End => {}
        NbtValue::Byte(b) => {
            out.push_str(&b.to_string());
            out.push('b');
        }
        NbtValue::Short(s) => {
            out.push_str(&s.to_string());
            out.push('s');
        }
        NbtValue::Int(i) => out.push_str(&i.to_string()),
        NbtValue::Long(l) => {
            out.push_str(&l.to_string());
            out.push('l');
        }
        NbtValue::Float(f) => {
            out.push_str(&f.to_string());
            out.push('f');
        }
        NbtValue::Double(d) => {
            out.push_str(&d.to_string());
            out.push('d');
        }
        NbtValue::ByteArray(xs) => {
            out.push_str("[B;");
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&x.to_string());
                out.push('B');
            }
            out.push(']');
        }
        NbtValue::String(s) => write_quoted(out, s, '"'),
        NbtValue::List(xs) => {
            out.push('[');
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_snbt_value(out, x);
            }
            out.push(']');
        }
        NbtValue::Compound(c) => out.push_str(&c.to_snbt()),
        NbtValue::IntArray(xs) => {
            out.push_str("[I;");
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&x.to_string());
                out.push('I');
            }
            out.push(']');
        }
        NbtValue::LongArray(xs) => {
            out.push_str("[L;");
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&x.to_string());
                out.push('L');
            }
            out.push(']');
        }
    }
}

struct SnbtParser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> SnbtParser<'a> {
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn parse_compound(&mut self) -> Result<NbtCompound, ()> {
        self.skip_ws();
        if !self.eat(b'{') {
            return Err(());
        }
        let mut out = NbtCompound::new();
        self.skip_ws();
        if self.eat(b'}') {
            return Ok(out);
        }
        loop {
            self.skip_ws();
            let key = self.parse_key()?;
            self.skip_ws();
            if !self.eat(b':') {
                return Err(());
            }
            self.skip_ws();
            let value = self.parse_value()?;
            out.put(key, value);
            self.skip_ws();
            if self.eat(b',') {
                continue;
            }
            if self.eat(b'}') {
                return Ok(out);
            }
            return Err(());
        }
    }

    // Quoted or unquoted (up to ':') keys, both allowed in SNBT.
    fn parse_key(&mut self) -> Result<String, ()> {
        match self.peek() {
            Some(b'"') => self.parse_quoted(b'"'),
            Some(b'\'') => self.parse_quoted(b'\''),
            Some(_) => {
                let start = self.i;
                while self.i < self.s.len() {
                    let c = self.s[self.i];
                    if c == b':' || c == b',' || c == b'}' || (c as char).is_whitespace() {
                        break;
                    }
                    self.i += 1;
                }
                if self.i == start {
                    return Err(());
                }
                // Unquoted keys are not escaped in SNBT.
                Ok(String::from_utf8_lossy(&self.s[start..self.i]).into_owned())
            }
            None => Err(()),
        }
    }

    fn parse_quoted(&mut self, quote: u8) -> Result<String, ()> {
        if !self.eat(quote) {
            return Err(());
        }
        let mut out = String::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            if c == b'\\' {
                let esc = self.peek().ok_or(())?;
                self.i += 1;
                match esc {
                    b'\\' => out.push('\\'),
                    b'"' => out.push('"'),
                    b'\'' => out.push('\''),
                    b'n' => out.push('\n'),
                    b't' => out.push('\t'),
                    b'r' => out.push('\r'),
                    b'f' => out.push('\u{000c}'),
                    b'b' => out.push('\u{0008}'),
                    _ => return Err(()),
                }
            } else if c == quote {
                return Ok(out);
            } else {
                out.push(c as char);
            }
        }
        Err(())
    }

    fn parse_value(&mut self) -> Result<NbtValue, ()> {
        self.skip_ws();
        match self.peek().ok_or(())? {
            b'{' => Ok(NbtValue::Compound(self.parse_compound()?)),
            b'[' => self.parse_list(),
            b'"' => Ok(NbtValue::String(self.parse_quoted(b'"')?)),
            b'\'' => Ok(NbtValue::String(self.parse_quoted(b'\'')?)),
            _ => {
                let token = self.parse_unquoted()?;
                Ok(snbt_scalar(&token))
            }
        }
    }

    fn parse_unquoted(&mut self) -> Result<String, ()> {
        let start = self.i;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c == b','
                || c == b'}'
                || c == b']'
                || c == b'{'
                || c == b'['
                || c == b':'
                || (c as char).is_whitespace()
            {
                break;
            }
            self.i += 1;
        }
        if self.i == start {
            return Err(());
        }
        Ok(String::from_utf8_lossy(&self.s[start..self.i]).into_owned())
    }

    fn parse_list(&mut self) -> Result<NbtValue, ()> {
        if !self.eat(b'[') {
            return Err(());
        }
        self.skip_ws();
        // Typed arrays: [B; ...], [I; ...], [L; ...]
        if let Some(kind) = self.peek() {
            if (kind == b'B' || kind == b'I' || kind == b'L')
                && self.s.get(self.i + 1) == Some(&b';')
            {
                self.i += 2;
                let mut xs = Vec::new();
                self.skip_ws();
                if self.eat(b']') {
                    return Ok(typed_array_value(kind, xs));
                }
                loop {
                    self.skip_ws();
                    let token = self.parse_unquoted()?;
                    let v = snbt_scalar(&token);
                    match (kind, &v) {
                        (b'B', NbtValue::Byte(x)) => xs.push(NbtValue::Byte(*x)),
                        (b'I', NbtValue::Int(x)) => xs.push(NbtValue::Int(*x)),
                        (b'L', NbtValue::Long(x)) => xs.push(NbtValue::Long(*x)),
                        (b'B', NbtValue::Int(x)) => xs.push(NbtValue::Byte(*x as i8)),
                        (b'I', NbtValue::Byte(x)) => xs.push(NbtValue::Int(*x as i32)),
                        (b'L', NbtValue::Int(x)) => xs.push(NbtValue::Long(*x as i64)),
                        _ => return Err(()),
                    }
                    self.skip_ws();
                    if self.eat(b',') {
                        continue;
                    }
                    if self.eat(b']') {
                        return Ok(typed_array_value(kind, xs));
                    }
                    return Err(());
                }
            }
        }
        let mut xs = Vec::new();
        self.skip_ws();
        if self.eat(b']') {
            return Ok(NbtValue::List(xs));
        }
        loop {
            self.skip_ws();
            xs.push(self.parse_value()?);
            self.skip_ws();
            if self.eat(b',') {
                continue;
            }
            if self.eat(b']') {
                return Ok(NbtValue::List(xs));
            }
            return Err(());
        }
    }
}

fn typed_array_value(kind: u8, xs: Vec<NbtValue>) -> NbtValue {
    match kind {
        b'B' => NbtValue::ByteArray(
            xs.into_iter()
                .map(|v| match v {
                    NbtValue::Byte(b) => b,
                    _ => 0,
                })
                .collect(),
        ),
        b'I' => NbtValue::IntArray(
            xs.into_iter()
                .map(|v| match v {
                    NbtValue::Int(i) => i,
                    _ => 0,
                })
                .collect(),
        ),
        _ => NbtValue::LongArray(
            xs.into_iter()
                .map(|v| match v {
                    NbtValue::Long(l) => l,
                    _ => 0,
                })
                .collect(),
        ),
    }
}

// Unquoted scalar tokens: booleans become bytes (vanilla StringNbtReader
// maps true/false to 1b/0b), suffixed numbers take their suffix, bare
// integers promote Int -> Long -> Double like vanilla, anything else is a
// plain string.
fn snbt_scalar(token: &str) -> NbtValue {
    if token == "true" {
        return NbtValue::Byte(1);
    }
    if token == "false" {
        return NbtValue::Byte(0);
    }
    let bytes = token.as_bytes();
    let (digits, suffix) = match bytes.last() {
        Some(&b'b') | Some(&b'B') => (&token[..token.len() - 1], Some(NbtValue::Byte(0))),
        Some(&b's') | Some(&b'S') => (&token[..token.len() - 1], Some(NbtValue::Short(0))),
        Some(&b'l') | Some(&b'L') => (&token[..token.len() - 1], Some(NbtValue::Long(0))),
        Some(&b'f') | Some(&b'F') => (&token[..token.len() - 1], Some(NbtValue::Float(0.0))),
        Some(&b'd') | Some(&b'D') => (&token[..token.len() - 1], Some(NbtValue::Double(0.0))),
        _ => (token, None),
    };
    if suffix.is_none() && !digits.is_empty() {
        if let Ok(i) = digits.parse::<i32>() {
            return NbtValue::Int(i);
        }
        if let Ok(l) = digits.parse::<i64>() {
            return NbtValue::Long(l);
        }
        if let Ok(d) = digits.parse::<f64>() {
            return NbtValue::Double(d);
        }
    }
    if !digits.is_empty() {
        match suffix {
            Some(NbtValue::Byte(_)) => {
                if let Ok(b) = digits.parse::<i8>() {
                    return NbtValue::Byte(b);
                }
            }
            Some(NbtValue::Short(_)) => {
                if let Ok(s) = digits.parse::<i16>() {
                    return NbtValue::Short(s);
                }
            }
            Some(NbtValue::Long(_)) => {
                if let Ok(l) = digits.parse::<i64>() {
                    return NbtValue::Long(l);
                }
            }
            Some(NbtValue::Float(_)) => {
                if let Ok(f) = digits.parse::<f32>() {
                    return NbtValue::Float(f);
                }
            }
            Some(NbtValue::Double(_)) => {
                if let Ok(d) = digits.parse::<f64>() {
                    return NbtValue::Double(d);
                }
            }
            _ => {}
        }
    }
    NbtValue::String(token.to_string())
}

// --- Binary NBT (PacketByteBuf.readNbt/writeNbt compound level) ---

pub const MAX_NBT_BYTES: i32 = 0x200000; // 2 MiB, vanilla PacketByteBuf cap

pub fn write_binary_nbt(out: &mut Vec<u8>, compound: &NbtCompound) {
    out.push(0x0a); // TAG_Compound
    write_binary_string(out, "");
    write_binary_payload(out, &NbtValue::Compound(compound.clone()));
    out.push(0x00); // TAG_End
}

fn write_binary_payload(out: &mut Vec<u8>, v: &NbtValue) {
    match v {
        NbtValue::End => {}
        NbtValue::Byte(b) => out.extend_from_slice(&b.to_be_bytes()),
        NbtValue::Short(s) => out.extend_from_slice(&s.to_be_bytes()),
        NbtValue::Int(i) => out.extend_from_slice(&i.to_be_bytes()),
        NbtValue::Long(l) => out.extend_from_slice(&l.to_be_bytes()),
        NbtValue::Float(f) => out.extend_from_slice(&f.to_be_bytes()),
        NbtValue::Double(d) => out.extend_from_slice(&d.to_be_bytes()),
        NbtValue::ByteArray(xs) => {
            out.extend_from_slice(&(xs.len() as i32).to_be_bytes());
            for x in xs {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
        NbtValue::String(s) => write_binary_string(out, s),
        NbtValue::List(xs) => {
            let kind = xs.first().map(nbt_kind).unwrap_or(0);
            out.push(kind);
            out.extend_from_slice(&(xs.len() as i32).to_be_bytes());
            for x in xs {
                write_binary_payload(out, x);
            }
        }
        NbtValue::Compound(c) => {
            for (k, v) in c.iter() {
                write_binary_tag(out, v, k);
            }
            out.push(0x00);
        }
        NbtValue::IntArray(xs) => {
            out.extend_from_slice(&(xs.len() as i32).to_be_bytes());
            for x in xs {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
        NbtValue::LongArray(xs) => {
            out.extend_from_slice(&(xs.len() as i32).to_be_bytes());
            for x in xs {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
    }
}

fn nbt_kind(v: &NbtValue) -> u8 {
    match v {
        NbtValue::End => 0,
        NbtValue::Byte(_) => 1,
        NbtValue::Short(_) => 2,
        NbtValue::Int(_) => 3,
        NbtValue::Long(_) => 4,
        NbtValue::Float(_) => 5,
        NbtValue::Double(_) => 6,
        NbtValue::ByteArray(_) => 7,
        NbtValue::String(_) => 8,
        NbtValue::List(_) => 9,
        NbtValue::Compound(_) => 10,
        NbtValue::IntArray(_) => 11,
        NbtValue::LongArray(_) => 12,
    }
}

fn write_binary_tag(out: &mut Vec<u8>, v: &NbtValue, name: &str) {
    let kind = nbt_kind(v);
    out.push(kind);
    write_binary_string(out, name);
    write_binary_payload(out, v);
}

fn write_binary_string(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u16).to_be_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn read_i16_at(bytes: &[u8], pos: usize) -> Result<i16, ()> {
    if bytes.len() < pos + 2 {
        return Err(());
    }
    Ok(i16::from_be_bytes([bytes[pos], bytes[pos + 1]]))
}

fn read_i32_at(bytes: &[u8], pos: usize) -> Result<i32, ()> {
    if bytes.len() < pos + 4 {
        return Err(());
    }
    Ok(i32::from_be_bytes([
        bytes[pos],
        bytes[pos + 1],
        bytes[pos + 2],
        bytes[pos + 3],
    ]))
}

fn read_i64_at(bytes: &[u8], pos: usize) -> Result<i64, ()> {
    if bytes.len() < pos + 8 {
        return Err(());
    }
    Ok(i64::from_be_bytes([
        bytes[pos],
        bytes[pos + 1],
        bytes[pos + 2],
        bytes[pos + 3],
        bytes[pos + 4],
        bytes[pos + 5],
        bytes[pos + 6],
        bytes[pos + 7],
    ]))
}

pub fn read_binary_nbt(bytes: &[u8]) -> Result<(NbtCompound, usize), ()> {
    let kind = *bytes.first().ok_or(())?;
    if kind != 0x0a {
        return Err(());
    }
    let (_name, n) = read_binary_string(bytes, 1)?;
    let pos = 1 + n;
    let (v, consumed) = read_binary_payload(bytes, pos, 0x0a)?;
    match v {
        NbtValue::Compound(c) => Ok((c, pos + consumed)),
        _ => Err(()),
    }
}

fn read_binary_string(bytes: &[u8], pos: usize) -> Result<(String, usize), ()> {
    if bytes.len() < pos + 2 {
        return Err(());
    }
    let len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
    if bytes.len() < pos + 2 + len {
        return Err(());
    }
    let s = bytes[pos + 2..pos + 2 + len]
        .iter()
        .map(|&b| b as char)
        .collect::<String>();
    Ok((s, 2 + len))
}

fn read_binary_payload(bytes: &[u8], pos: usize, kind: u8) -> Result<(NbtValue, usize), ()> {
    match kind {
        0 => Ok((NbtValue::End, 0)),
        1 => {
            let b = *bytes.get(pos).ok_or(())?;
            Ok((NbtValue::Byte(b as i8), 1))
        }
        2 => {
            let s = read_i16_at(bytes, pos)?;
            Ok((NbtValue::Short(s), 2))
        }
        3 => {
            let i = read_i32_at(bytes, pos)?;
            Ok((NbtValue::Int(i), 4))
        }
        4 => {
            let l = read_i64_at(bytes, pos)?;
            Ok((NbtValue::Long(l), 8))
        }
        5 => {
            let f = f32::from_bits(read_i32_at(bytes, pos)? as u32);
            Ok((NbtValue::Float(f), 4))
        }
        6 => {
            let d = f64::from_bits(read_i64_at(bytes, pos)? as u64);
            Ok((NbtValue::Double(d), 8))
        }
        8 => {
            let (s, n) = read_binary_string(bytes, pos)?;
            Ok((NbtValue::String(s), n))
        }
        10 => {
            let mut out = NbtCompound::new();
            let mut p = pos;
            loop {
                let kind = *bytes.get(p).ok_or(())?;
                if kind == 0 {
                    return Ok((NbtValue::Compound(out), p + 1 - pos));
                }
                p += 1;
                let (name, n) = read_binary_string(bytes, p)?;
                p += n;
                let (v, n) = read_binary_payload(bytes, p, kind)?;
                p += n;
                out.put(name, v);
            }
        }
        9 => {
            let kind = *bytes.get(pos).ok_or(())?;
            let len = read_i32_at(bytes, pos + 1)? as usize;
            let mut p = pos + 5;
            let mut xs = Vec::new();
            for _ in 0..len {
                let (v, n) = read_binary_payload(bytes, p, kind)?;
                p += n;
                xs.push(v);
            }
            Ok((NbtValue::List(xs), p - pos))
        }
        7 | 11 | 12 => {
            let len = read_i32_at(bytes, pos)? as usize;
            let w = if kind == 7 {
                1
            } else if kind == 11 {
                4
            } else {
                8
            };
            let mut xs = Vec::new();
            let mut p = pos + 4;
            for _ in 0..len {
                let raw = bytes.get(p..p + w).ok_or(())?;
                xs.push(match kind {
                    7 => NbtValue::Byte(raw[0] as i8),
                    11 => NbtValue::Int(i32::from_be_bytes(raw.try_into().unwrap())),
                    _ => NbtValue::Long(i64::from_be_bytes(raw.try_into().unwrap())),
                });
                p += w;
            }
            Ok((
                match kind {
                    7 => NbtValue::ByteArray(
                        xs.into_iter()
                            .map(|v| match v {
                                NbtValue::Byte(b) => b,
                                _ => unreachable!(),
                            })
                            .collect(),
                    ),
                    11 => NbtValue::IntArray(
                        xs.into_iter()
                            .map(|v| match v {
                                NbtValue::Int(i) => i,
                                _ => unreachable!(),
                            })
                            .collect(),
                    ),
                    _ => NbtValue::LongArray(
                        xs.into_iter()
                            .map(|v| match v {
                                NbtValue::Long(l) => l,
                                _ => unreachable!(),
                            })
                            .collect(),
                    ),
                },
                p - pos,
            ))
        }
        _ => Err(()),
    }
}

// ---------------------------------------------------------------------------
// Text and ItemStack
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum HoverEvent {
    ShowItem(ItemStack),
}

// Only the surface this mod touches: literal text with an optional hover
// event (the hover is carried for parity; getString() is the plain content).
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub plain: String,
    pub hover: Option<HoverEvent>,
}

impl Text {
    pub fn literal(s: impl Into<String>) -> Self {
        Text {
            plain: s.into(),
            hover: None,
        }
    }

    pub fn get_string(&self) -> &str {
        &self.plain
    }
}

// The display component stored in tag.display.Name is a JSON text component;
// getString() of a component is its text content. This port resolves the
// plain form of simple components (a bare string, or {"text": "..."}) and
// falls back to the humanized item id when the tag carries no display name,
// which is where vanilla consults the translation registry.
pub fn plain_text_of_component(json: &str) -> Option<String> {
    let json = json.trim();
    if !json.starts_with('{') {
        return Some(unquote(json));
    }
    let v = serde_json::from_str::<serde_json::Value>(json).ok()?;
    if let Some(text) = v.get("text").and_then(|t| t.as_str()) {
        return Some(text.to_string());
    }
    None
}

fn unquote(s: &str) -> String {
    let bytes = s.as_bytes();
    if bytes.len() >= 2
        && (bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"'
            || bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'')
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn humanize_item_id(id: &str) -> String {
    let path = match id.split_once(':') {
        Some((_, path)) => path,
        None => id,
    };
    let mut out = String::new();
    for (i, part) in path.split('_').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ItemStack {
    pub id: String,
    pub count: i32,
    pub tag: Option<NbtCompound>,
}

impl ItemStack {
    // ItemStack.fromNbt: "id" (required, missing means the read fails), the
    // "Count" int (vanilla reads "Count" with a 1 default), and the rest of
    // the compound as the item's tag.
    pub fn from_nbt(nbt: &NbtCompound) -> Option<Self> {
        let id = nbt.get_string("id")?;
        let count = nbt.get_int("Count").unwrap_or(1);
        Some(ItemStack {
            id: id.to_string(),
            count,
            tag: nbt.get_compound("tag").cloned(),
        })
    }

    pub fn write_nbt(&self) -> NbtCompound {
        let mut nbt = NbtCompound::new();
        nbt.put("id", NbtValue::String(self.id.clone()));
        nbt.put("Count", NbtValue::Int(self.count));
        if let Some(tag) = &self.tag {
            nbt.put("tag", NbtValue::Compound(tag.clone()));
        }
        nbt
    }

    pub fn has_nbt(&self) -> bool {
        self.tag.is_some()
    }

    pub fn nbt(&self) -> Option<&NbtCompound> {
        self.tag.as_ref()
    }

    pub fn is_empty(&self) -> bool {
        self.id.is_empty() || self.count == 0
    }

    pub fn display_name(&self) -> String {
        if let Some(name) = self
            .tag
            .as_ref()
            .and_then(|t| t.get_compound("display"))
            .and_then(|d| d.get_string("Name"))
            .and_then(plain_text_of_component)
        {
            return name;
        }
        humanize_item_id(&self.id)
    }

    // ItemStack.toHoverableText(): a text component carrying the display name
    // with a SHOW_ITEM hover event.
    pub fn to_hoverable_text(&self) -> Text {
        Text {
            plain: self.display_name(),
            hover: Some(HoverEvent::ShowItem(self.clone())),
        }
    }

    pub fn to_hoverable_text_string(&self) -> String {
        self.to_hoverable_text().get_string().to_string()
    }
}

// ---------------------------------------------------------------------------
// Core: SmwygItemMatch, the tag regex, extraction, decoration
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Default)]
pub struct SmwygItemMatch {
    // Java's field starts null; a match with a stack that failed to parse
    // keeps its span and renders the stale-link placeholder instead.
    pub stack: Option<ItemStack>,
    pub start: i32,
    pub end: i32,
}

// Java Pattern "\\[\\[smwyg:(?<item>.*)\\]\\]": greedy `.*` that never spans
// a line break, so within a line the item group is everything after the
// opening tag up to the LAST "]]" on that line. find() retries at each
// following position until a start yields a match. Indices are char indices
// (Java's are UTF-16 code units; identical for BMP text, which is what chat
// strings carry).
fn find_tag_in_line(
    line: &str,
    line_start: usize,
    open_abs: usize,
) -> Option<(usize, usize, String)> {
    let item_from = open_abs - line_start + TAG_OPEN.len();
    let rest = &line[item_from.min(line.len())..];
    // last occurrence of "]]"
    let mut last = None;
    let rb: Vec<char> = rest.chars().collect();
    for i in 0..rb.len().saturating_sub(1) {
        if rb[i] == ']' && rb[i + 1] == ']' {
            last = Some(i);
        }
    }
    let last = last?;
    let item: String = rb[..last].iter().collect();
    let end_abs =
        open_abs + TAG_OPEN.len() + rb[..last + 2].iter().map(|c| c.len_utf8()).sum::<usize>();
    Some((open_abs, end_abs, item))
}

fn find_tag(text: &str) -> Option<(usize, usize, String)> {
    let mut search = 0usize;
    while search <= text.len() {
        let hay = &text[search..];
        let off = match hay.find(TAG_OPEN) {
            Some(o) => o + search,
            None => return None,
        };
        // Determine the line containing the opening tag.
        let line_start = text[..off].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line_end = text[off..]
            .find('\n')
            .map(|i| off + i)
            .unwrap_or(text.len());
        if let Some((s, e, item)) = find_tag_in_line(&text[line_start..line_end], line_start, off) {
            return Some((s, e, item));
        }
        search = off + 1;
    }
    None
}

pub fn has_smwyg_item(text: &str) -> bool {
    find_tag(text).is_some()
}

// extractItem: parse the SNBT compound out of the tag; a parse failure keeps
// the span with a null stack (CommandSyntaxException is swallowed upstream).
pub fn extract_item(text: &str) -> Option<SmwygItemMatch> {
    let (start, end, item_nbt) = find_tag(text)?;
    let stack = NbtCompound::parse_snbt(&item_nbt)
        .ok()
        .and_then(|nbt| ItemStack::from_nbt(&nbt));
    Some(SmwygItemMatch {
        stack,
        start: start as i32,
        end: end as i32,
    })
}

// decorateMessage: message.getString() with the [start, end) span replaced by
// the stack's hoverable text.
pub fn decorate_message(message: &str, item_match: Option<&SmwygItemMatch>) -> Text {
    let Some(item_match) = item_match else {
        return Text::literal(message);
    };
    let Some(stack) = &item_match.stack else {
        return Text::literal(message);
    };
    let mut out = String::new();
    out.push_str(&slice_chars(message, 0, item_match.start as usize));
    out.push_str(&stack.to_hoverable_text_string());
    out.push_str(&slice_chars(
        message,
        item_match.end as usize,
        i32::MAX as usize,
    ));
    Text {
        plain: out,
        hover: None,
    }
}

// Java String.substring on char (UTF-16) indices; ports slice by chars.
fn slice_chars(s: &str, from: usize, to: usize) -> String {
    let total = s.chars().count();
    let from = from.min(total);
    let to = to.min(total);
    if from >= to {
        return String::new();
    }
    s.chars().skip(from).take(to - from).collect()
}

// DrawContextMixin's hide check: an NBT boolean on the shared stack suppresses
// the tooltip item (render-half gate; the predicate is pure and shared).
pub fn should_hide_stack(stack: &ItemStack) -> bool {
    stack
        .nbt()
        .map(|nbt| nbt.get_boolean(HIDE_STACK_NBT))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// MessageItemList (per-sender matches; take-on-read; cleared on server start)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct MessageItemList {
    shared_items: HashMap<u64, SmwygItemMatch>,
}

impl MessageItemList {
    pub fn new() -> Self {
        Self::default()
    }

    // ServerLifecycleEvents.SERVER_STARTED registration in the constructor.
    pub fn on_server_started(&mut self) {
        self.shared_items.clear();
    }

    pub fn add(&mut self, sender: u64, item_match: SmwygItemMatch) {
        self.shared_items.insert(sender, item_match);
    }

    // get() removes the stored match when present (containsKey + remove).
    pub fn take(&mut self, sender: u64) -> Option<SmwygItemMatch> {
        self.shared_items.remove(&sender)
    }
}

// ---------------------------------------------------------------------------
// Share-item packet + server receiver
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub struct ShareItemPacket {
    pub start: i32,
    pub end: i32,
    pub stack: ItemStack,
}

// PacketByteBuf framing: two varints then the compound NBT.
pub struct PacketBuf {
    pub bytes: Vec<u8>,
    pub pos: usize,
}

impl PacketBuf {
    pub fn new() -> Self {
        PacketBuf {
            bytes: Vec::new(),
            pos: 0,
        }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        PacketBuf { bytes, pos: 0 }
    }

    pub fn write_varint(&mut self, mut value: i32) {
        loop {
            if (value & !0x7f) == 0 {
                self.bytes.push(value as u8);
                return;
            }
            self.bytes.push(((value & 0x7f) | 0x80) as u8);
            value = ((value as u32) >> 7) as i32;
        }
    }

    pub fn read_varint(&mut self) -> Result<i32, ()> {
        let mut result: u32 = 0;
        for shift in [0u32, 7, 14, 21, 28] {
            let b = *self.bytes.get(self.pos).ok_or(())?;
            self.pos += 1;
            result |= ((b & 0x7f) as u32) << shift;
            if b & 0x80 == 0 {
                return Ok(result as i32);
            }
        }
        Err(())
    }

    pub fn write_packet(&mut self, packet: &ShareItemPacket) {
        self.write_varint(packet.start);
        self.write_varint(packet.end);
        write_binary_nbt(&mut self.bytes, &packet.stack.write_nbt());
    }

    pub fn read_packet(&mut self) -> Result<ShareItemPacket, ()> {
        let start = self.read_varint()?;
        let end = self.read_varint()?;
        let (nbt, _) = read_binary_nbt(&self.bytes[self.pos..])?;
        let stack = ItemStack::from_nbt(&nbt).ok_or(())?;
        self.pos = self.bytes.len();
        Ok(ShareItemPacket { start, end, stack })
    }
}

impl Default for PacketBuf {
    fn default() -> Self {
        Self::new()
    }
}

// Client side: ShowMeWhatYouGotClient.sendItemSharingMessage builds the
// buffer and sends it on the share_item channel.
pub fn send_item_sharing_message(start: i32, end: i32, stack: &ItemStack) -> PacketBuf {
    let mut buf = PacketBuf::new();
    buf.write_packet(&ShareItemPacket {
        start,
        end,
        stack: stack.clone(),
    });
    buf
}

// The server receiver registers a global handler that reads the packet and
// hands the match to the server's execute queue (minecraftServer.execute).
#[derive(Default)]
pub struct Server {
    tasks: Vec<(u64, SmwygItemMatch)>,
}

impl Server {
    pub fn execute(&mut self, player: u64, item_match: SmwygItemMatch) {
        self.tasks.push((player, item_match));
    }

    pub fn drain_execute(&mut self, list: &mut MessageItemList) {
        for (player, item_match) in self.tasks.drain(..) {
            list.add(player, item_match);
        }
    }
}

// ServerPlayNetworking receiver body: read start/end/NBT, then enqueue the
// SmwygItemMatch for the player.
pub fn receive_share_item_packet(
    server: &mut Server,
    player: u64,
    buf: &mut PacketBuf,
) -> Result<(), ()> {
    let packet = buf.read_packet()?;
    server.execute(
        player,
        SmwygItemMatch {
            stack: Some(packet.stack),
            start: packet.start,
            end: packet.end,
        },
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Client: tick share gate + chat screen wiring
// ---------------------------------------------------------------------------

pub struct SlotSnapshot {
    pub has_stack: bool,
    pub stack: ItemStack,
}

pub struct TickSnapshot {
    pub player_present: bool,
    pub current_screen_is_handled: bool,
    pub focused_slot: Option<SlotSnapshot>,
    pub cursor_stack_empty: bool,
    pub left_control_pressed: bool,
    pub chat_key_pressed: bool,
}

pub struct ClientState {
    pub shared_stack: bool,
    pub sharing_item: Option<ItemStack>,
}

impl Default for ClientState {
    fn default() -> Self {
        ClientState {
            shared_stack: false,
            sharing_item: None,
        }
    }
}

pub struct TickOutcome {
    pub open_chat_with: Option<String>,
    pub shared_stack: bool,
}

// ClientTickEvents.START_CLIENT_TICK body: the sharedStack latch is set on
// the ctrl+chat press BEFORE the open-chat checks (so a press with a full
// cursor or no focused slot still latches until the keys release), and the
// chat opens with the focused slot's hoverable text.
pub fn client_tick(state: &mut ClientState, s: TickSnapshot) -> TickOutcome {
    let mut open_chat_with = None;
    if s.player_present && s.current_screen_is_handled {
        if s.left_control_pressed && s.chat_key_pressed && !state.shared_stack {
            state.shared_stack = true;
            if s.cursor_stack_empty && s.focused_slot.as_ref().is_some_and(|f| f.has_stack) {
                let stack = s.focused_slot.as_ref().unwrap().stack.clone();
                state.sharing_item = Some(stack.clone());
                open_chat_with = Some(stack.to_hoverable_text_string());
            }
        }
        if state.shared_stack && (!s.left_control_pressed || !s.chat_key_pressed) {
            state.shared_stack = false;
        }
    }
    TickOutcome {
        open_chat_with,
        shared_stack: state.shared_stack,
    }
}

// ChatScreen.init inject: apply the pending sharing item to the field and
// clear it, before the field's setText runs (handle_item_setting then passes
// the text through because the stack is already set).
pub fn chat_screen_init(client: &mut ClientState, field: &mut ItemSharingTextFieldWidget) -> bool {
    if let Some(stack) = client.sharing_item.take() {
        field.set_stack(stack);
        true
    } else {
        false
    }
}

// ChatScreen.setChatFromHistory inject: any history navigation resets the
// inserted item.
pub fn chat_screen_set_chat_from_history(field: &mut ItemSharingTextFieldWidget) {
    field.reset();
}

pub enum SendMessageOutcome {
    Normal {
        add_to_history: bool,
    },
    Shared {
        // The original field text still goes through vanilla sendMessage.
        sent_text: String,
        // The tagged version added to the local chat history instead.
        history_text: String,
        packet: PacketBuf,
    },
}

// ChatScreen.sendMessage ModifyVariable: with a sharing stack, send the
// packet for the insertion span, add the [[smwyg:...]]-tagged text to the
// history, and pass addToHistory=false down (no duplicate untagged history
// entry). Without one, vanilla behavior.
pub fn chat_send_message(
    field: &ItemSharingTextFieldWidget,
    chat_text: &str,
    add_to_history: bool,
) -> SendMessageOutcome {
    let Some(stack) = field.get_stack() else {
        return SendMessageOutcome::Normal { add_to_history };
    };
    let before = field.get_text_before(chat_text);
    let after = field.get_text_after(chat_text);
    let packet = send_item_sharing_message(
        field.get_insertion_start(),
        field.get_insertion_end(),
        stack,
    );
    let stack_string = format!("{}{}{}", TAG_OPEN, stack.write_nbt().to_snbt(), TAG_CLOSE);
    SendMessageOutcome::Shared {
        sent_text: chat_text.to_string(),
        history_text: format!("{}{}{}", before, stack_string, after),
        packet,
    }
}

// ---------------------------------------------------------------------------
// Shared text field: the ItemSharingTextFieldWidget state machine
// (TextFieldWidgetMixin + duck, index math ported 1:1 with i32 to keep Java's
// signed int semantics; string slices use char indices like the rest)
// ---------------------------------------------------------------------------

#[derive(Default, Clone, Debug)]
pub struct ItemSharingTextFieldWidget {
    item_stack: Option<ItemStack>,
    inserted_string: String,
    inserted_index: i32,
    inserted_length: i32,
}

pub struct WriteAdjustment {
    pub text: String,
    pub selection_start: i32,
    pub selection_end: i32,
}

impl ItemSharingTextFieldWidget {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_stack(&mut self, stack: ItemStack) {
        self.inserted_string = stack.to_hoverable_text_string();
        self.inserted_length = self.inserted_string.chars().count() as i32;
        self.item_stack = Some(stack);
    }

    pub fn get_stack(&self) -> Option<&ItemStack> {
        self.item_stack.as_ref()
    }

    pub fn has_stack(&self) -> bool {
        self.item_stack.is_some()
    }

    pub fn reset(&mut self) {
        self.item_stack = None;
        self.inserted_string = String::new();
        self.inserted_index = 0;
        self.inserted_length = 0;
    }

    pub fn get_insertion_start(&self) -> i32 {
        self.inserted_index
    }

    pub fn get_insertion_end(&self) -> i32 {
        self.inserted_index + self.inserted_length
    }

    pub fn get_text_before(&self, text: &str) -> String {
        slice_chars(text, 0, self.inserted_index.max(0) as usize)
    }

    pub fn get_text_after(&self, text: &str) -> String {
        slice_chars(
            text,
            (self.inserted_index + self.inserted_length).max(0) as usize,
            i32::MAX as usize,
        )
    }

    pub fn on_suggestion_inserted(&mut self, start: i32, offset: i32) {
        if self.item_stack.is_some() && start <= self.inserted_index {
            self.inserted_index += offset;
        }
    }

    // getCursorPosWithOffset RETURN override: Some replaces the cursor, None
    // keeps the vanilla value.
    pub fn modify_cursor_offset(&self, original: i32, offset: i32) -> Option<i32> {
        if original > self.inserted_index && original < self.inserted_index + self.inserted_length {
            if offset < 0 {
                Some(self.inserted_index)
            } else {
                Some(self.inserted_index + self.inserted_length)
            }
        } else {
            None
        }
    }

    // write() HEAD inject: writing a selection that covers the insertion
    // removes the inserted span, shifts the selection past the old text, and
    // resets. None means vanilla write proceeds untouched.
    pub fn remove_stack_when_overwritten(
        &mut self,
        text: &str,
        selection_start: i32,
        selection_end: i32,
    ) -> Option<WriteAdjustment> {
        let i = selection_start.min(selection_end);
        let j = selection_start.max(selection_end);
        if i <= self.inserted_index && j > self.inserted_index {
            let chars: Vec<char> = text.chars().collect();
            let total = chars.len() as i32;
            let from = self.inserted_index.max(0).min(total);
            let to = (self.inserted_index + self.inserted_length)
                .max(0)
                .min(total);
            let new_text: String = chars[..from as usize]
                .iter()
                .chain(chars[to as usize..].iter())
                .collect();
            let (new_start, new_end) = if selection_start > selection_end {
                (selection_start - self.inserted_length, selection_end)
            } else {
                (selection_start, selection_end - self.inserted_length)
            };
            self.reset();
            Some(WriteAdjustment {
                text: new_text,
                selection_start: new_start,
                selection_end: new_end,
            })
        } else {
            None
        }
    }

    // write() setSelectionStart inject: i/j are the ordered selection bounds
    // and l the length delta; the insertion index moves with writes entirely
    // left of it.
    pub fn move_inserted_text(&mut self, i: i32, j: i32, l: i32) {
        if i <= self.inserted_index && j <= self.inserted_index {
            self.inserted_index += l;
        }
    }

    // setCursor HEAD override.
    pub fn modify_cursor_setting(&self, original: i32) -> i32 {
        if original > self.inserted_index && original < self.inserted_index + self.inserted_length {
            if original <= self.inserted_index + self.inserted_length / 2 {
                self.inserted_index
            } else {
                self.inserted_index + self.inserted_length
            }
        } else {
            original
        }
    }

    pub fn modify_selection_start(&self, original: i32) -> i32 {
        if original > self.inserted_index && original < self.inserted_index + self.inserted_length {
            self.inserted_index
        } else {
            original
        }
    }

    pub fn modify_selection_end(&self, original: i32) -> i32 {
        if original > self.inserted_index && original < self.inserted_index + self.inserted_length {
            self.inserted_index
        } else {
            original
        }
    }

    // eraseCharacters inject at the StringBuilder build: j/k are the erase
    // range bounds as vanilla computes them. Returns whether the whole
    // insertion was erased (which resets the widget).
    pub fn erase_insertion(&mut self, j: i32, k: i32) -> bool {
        if j <= self.inserted_index && k >= self.inserted_index + self.inserted_length {
            self.reset();
            true
        } else {
            if j <= self.inserted_index || k <= self.inserted_index {
                self.inserted_index -= k - j;
            }
            false
        }
    }

    // setText HEAD override: when no stack is held, a text containing a
    // [[smwyg:...]] tag is rewritten with the item's display name and the
    // stack is adopted (chat history recall); a tag that fails to parse
    // becomes the stale-link placeholder. Without any tag the widget resets.
    pub fn handle_item_setting(&mut self, text: &str) -> String {
        if self.has_stack() {
            return text.to_string();
        }
        if has_smwyg_item(text) {
            let item_match = extract_item(text).unwrap();
            let before = slice_chars(text, 0, item_match.start as usize);
            let after = slice_chars(text, item_match.end as usize, i32::MAX as usize);
            match item_match.stack {
                None => {
                    self.reset();
                    format!("{}{}{}", before, STALE_LINK_TEXT, after)
                }
                Some(stack) => {
                    let item_text = stack.to_hoverable_text_string();
                    self.item_stack = Some(stack);
                    self.inserted_string = item_text.clone();
                    self.inserted_index = item_match.start;
                    self.inserted_length = item_text.chars().count() as i32;
                    format!("{}{}{}", before, item_text, after)
                }
            }
        } else {
            self.reset();
            text.to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sword_stack(count: i32) -> ItemStack {
        let mut tag = NbtCompound::new();
        let mut display = NbtCompound::new();
        display.put("Name", NbtValue::String("\"Cool Sword\"".to_string()));
        tag.put("display", NbtValue::Compound(display));
        ItemStack {
            id: "minecraft:diamond_sword".to_string(),
            count,
            tag: Some(tag),
        }
    }

    fn plain_stack(id: &str, count: i32) -> ItemStack {
        ItemStack {
            id: id.to_string(),
            count,
            tag: None,
        }
    }

    #[test]
    fn test_snbt_compound_parse() {
        let nbt = NbtCompound::parse_snbt("{id:\"minecraft:diamond_sword\",Count:1b}").unwrap();
        assert_eq!(nbt.get_string("id"), Some("minecraft:diamond_sword"));
        assert_eq!(nbt.get_int("Count"), None); // 1b is a Byte, not an Int
        assert_eq!(nbt.get_boolean("Count"), true);
    }

    #[test]
    fn test_snbt_roundtrip() {
        let mut inner = NbtCompound::new();
        inner.put("Damage", NbtValue::Int(7));
        inner.put("Name", NbtValue::String("a\"b".to_string()));
        let mut outer = NbtCompound::new();
        outer.put("tag", NbtValue::Compound(inner));
        outer.put("flag", NbtValue::Byte(1));
        let snbt = outer.to_snbt();
        assert_eq!(NbtCompound::parse_snbt(&snbt).unwrap(), outer);
    }

    #[test]
    fn test_snbt_scalar_semantics() {
        let nbt =
            NbtCompound::parse_snbt("{a:true,b:false,c:1.5,d:9223372036854775807,e:hi}").unwrap();
        assert_eq!(nbt.get_boolean("a"), true);
        assert_eq!(nbt.get_boolean("b"), false);
        assert!(matches!(nbt.get("c"), Some(NbtValue::Double(_))));
        assert!(matches!(nbt.get("d"), Some(NbtValue::Long(_))));
        assert_eq!(nbt.get_string("e"), Some("hi"));
    }

    #[test]
    fn test_snbt_parse_failure() {
        assert!(NbtCompound::parse_snbt("not a compound").is_err());
        assert!(NbtCompound::parse_snbt("{unclosed").is_err());
    }

    #[test]
    fn test_item_stack_nbt_roundtrip() {
        let stack = sword_stack(3);
        let nbt = stack.write_nbt();
        assert_eq!(ItemStack::from_nbt(&nbt).unwrap(), stack);
    }

    #[test]
    fn test_display_name_prefers_component() {
        assert_eq!(sword_stack(1).display_name(), "Cool Sword");
        assert_eq!(
            plain_stack("minecraft:golden_apple", 1).display_name(),
            "Golden Apple"
        );
    }

    #[test]
    fn test_find_tag_greedy() {
        let text =
            "hey [[smwyg:{id:\"minecraft:stone\"}]] and [[smwyg:{id:\"minecraft:dirt\"}]] lol";
        let m = extract_item(text).unwrap();
        assert_eq!(m.start, 4);
        // greedy .* backtracks to the LAST "]]" on the line
        assert_eq!(text[m.start as usize..m.end as usize].ends_with("]]"), true);
        // StringNbtReader parses the leading compound and ignores the
        // trailing text after it, so the stack is the FIRST item
        assert_eq!(m.stack.as_ref().unwrap().id, "minecraft:stone");
    }

    #[test]
    fn test_find_tag_multiline_and_absent() {
        assert_eq!(has_smwyg_item("no tag here"), false);
        // tag open on one line, close on the next: `.*` cannot cross \n
        let split = "a [[smwyg:{id:\"minecraft:stone\"}\n]] b";
        assert_eq!(has_smwyg_item(split), false);
        assert_eq!(has_smwyg_item("[[smwyg:{}]]"), true);
    }

    #[test]
    fn test_extract_item_malformed_keeps_span() {
        let m = extract_item("[[smwyg:{not:nbt]]").unwrap();
        assert!(m.stack.is_none());
        assert!(m.end > m.start);
    }

    #[test]
    fn test_decorate_message() {
        let stack = sword_stack(1);
        let text = "look [[smwyg:{id:\"minecraft:diamond_sword\",tag:{display:{Name:\"Cool Sword\"}}}]] now";
        let m = extract_item(text).unwrap();
        let out = decorate_message(text, Some(&m));
        assert_eq!(out.get_string(), "look Cool Sword now");
        assert!(matches!(out.hover, None));
        assert_eq!(decorate_message(text, None).get_string(), text);
    }

    #[test]
    fn test_message_list_take_and_clear() {
        let mut list = MessageItemList::new();
        let m1 = SmwygItemMatch {
            stack: Some(plain_stack("minecraft:stone", 1)),
            start: 0,
            end: 1,
        };
        list.add(7, m1.clone());
        assert_eq!(list.take(7).unwrap(), m1);
        assert_eq!(list.take(7), None); // take-on-read like upstream get()
        list.add(7, m1);
        list.on_server_started();
        assert_eq!(list.take(7), None);
    }

    #[test]
    fn test_packet_roundtrip() {
        let mut buf = PacketBuf::new();
        buf.write_packet(&ShareItemPacket {
            start: 12,
            end: 40,
            stack: sword_stack(2),
        });
        let mut read = PacketBuf::from_bytes(buf.bytes.clone());
        let p = read.read_packet().unwrap();
        assert_eq!(p.start, 12);
        assert_eq!(p.end, 40);
        assert_eq!(p.stack, sword_stack(2));
    }

    #[test]
    fn test_server_receiver_deferred_add() {
        let mut server = Server::default();
        let mut list = MessageItemList::new();
        let mut buf = PacketBuf::new();
        buf.write_packet(&ShareItemPacket {
            start: 0,
            end: 10,
            stack: sword_stack(1),
        });
        receive_share_item_packet(&mut server, 42, &mut buf).unwrap();
        // nothing lands until the server executes its task queue
        assert_eq!(list.take(42), None);
        server.drain_execute(&mut list);
        let m = list.take(42).unwrap();
        assert_eq!(m.stack.unwrap().id, "minecraft:diamond_sword");
        assert_eq!(m.start, 0);
        assert_eq!(m.end, 10);
    }

    #[test]
    fn test_client_tick_gate() {
        let mut state = ClientState::default();
        let base = |slot: Option<SlotSnapshot>| TickSnapshot {
            player_present: true,
            current_screen_is_handled: true,
            focused_slot: slot,
            cursor_stack_empty: true,
            left_control_pressed: true,
            chat_key_pressed: true,
        };
        // press with a focused slot opens the chat and latches
        let out = client_tick(
            &mut state,
            base(Some(SlotSnapshot {
                has_stack: true,
                stack: sword_stack(1),
            })),
        );
        assert_eq!(out.open_chat_with.as_deref(), Some("Cool Sword"));
        assert!(state.sharing_item.is_some());
        assert!(out.shared_stack);
        // still pressed: no second open
        let out = client_tick(
            &mut state,
            base(Some(SlotSnapshot {
                has_stack: true,
                stack: sword_stack(1),
            })),
        );
        assert!(out.open_chat_with.is_none());
        // release resets the latch
        let mut s = base(Some(SlotSnapshot {
            has_stack: true,
            stack: sword_stack(1),
        }));
        s.left_control_pressed = false;
        let out = client_tick(&mut state, s);
        assert!(out.open_chat_with.is_none());
        assert!(!out.shared_stack);
        // press with a non-empty cursor latches but does not open
        let mut s = base(Some(SlotSnapshot {
            has_stack: true,
            stack: sword_stack(1),
        }));
        s.cursor_stack_empty = false;
        let out = client_tick(&mut state, s);
        assert!(out.open_chat_with.is_none());
        assert!(out.shared_stack);
    }

    #[test]
    fn test_text_field_set_and_bounds() {
        let mut field = ItemSharingTextFieldWidget::new();
        field.set_stack(sword_stack(1));
        assert!(field.has_stack());
        assert_eq!(field.get_insertion_start(), 0);
        assert_eq!(
            field.get_insertion_end(),
            "Cool Sword".chars().count() as i32
        );
        let text = "Cool Sword now";
        assert_eq!(field.get_text_before(text), "");
        assert_eq!(field.get_text_after(text), " now");
    }

    #[test]
    fn test_cursor_offset_and_setting() {
        let mut field = ItemSharingTextFieldWidget::new();
        field.set_stack(sword_stack(1));
        field.inserted_index = 3;
        let len = field.inserted_length;
        // inside the span: arrow keys jump to the span edges
        assert_eq!(field.modify_cursor_offset(4, -1), Some(3));
        assert_eq!(field.modify_cursor_offset(4, 1), Some(3 + len));
        assert_eq!(field.modify_cursor_offset(2, 1), None);
        // setCursor splits at the midpoint (len/2, integer division)
        assert_eq!(field.modify_cursor_setting(3 + len / 2 + 1), 3 + len);
        assert_eq!(field.modify_cursor_setting(3 + len / 2), 3);
        assert_eq!(field.modify_cursor_setting(1), 1);
        assert_eq!(field.modify_selection_start(3 + 1), 3);
        assert_eq!(field.modify_selection_end(3 + len - 1), 3);
    }

    #[test]
    fn test_remove_stack_when_overwritten() {
        let mut field = ItemSharingTextFieldWidget::new();
        field.set_stack(sword_stack(1));
        field.inserted_index = 2;
        let len = field.inserted_length;
        let text = "ab Cool Sword cd";
        let adj = field.remove_stack_when_overwritten(text, 1, 5).unwrap();
        // removal span [2, 12) of "ab Cool Sword cd"
        assert_eq!(adj.text, "abd cd");
        assert_eq!(adj.selection_start, 1);
        assert_eq!(adj.selection_end, 5 - len);
        assert!(!field.has_stack());
        // selection not covering the insertion: untouched
        let mut field2 = ItemSharingTextFieldWidget::new();
        field2.set_stack(sword_stack(1));
        field2.inserted_index = 5;
        assert!(field2.remove_stack_when_overwritten(text, 0, 2).is_none());
        assert!(field2.has_stack());
    }

    #[test]
    fn test_move_inserted_text_and_erase() {
        let mut field = ItemSharingTextFieldWidget::new();
        field.set_stack(sword_stack(1));
        field.inserted_index = 5;
        field.move_inserted_text(0, 2, 3); // write left of the insertion
        assert_eq!(field.inserted_index, 8);
        field.move_inserted_text(6, 9, 2); // write overlapping the insertion
        assert_eq!(field.inserted_index, 8);
        // erase covering the whole insertion resets
        let end = field.get_insertion_end();
        assert!(field.erase_insertion(8, end));
        assert_eq!(field.inserted_index, 0);
        // partial erase left of the insertion shifts it
        let mut field2 = ItemSharingTextFieldWidget::new();
        field2.set_stack(sword_stack(1));
        field2.inserted_index = 4;
        assert!(!field2.erase_insertion(1, 3));
        assert_eq!(field2.inserted_index, 2);
    }

    #[test]
    fn test_handle_item_setting_flow() {
        let mut field = ItemSharingTextFieldWidget::new();
        // no tag: reset, text unchanged
        assert_eq!(field.handle_item_setting("plain text"), "plain text");
        assert!(!field.has_stack());
        // valid tag: display name spliced in, stack adopted
        let tagged = "look [[smwyg:{id:\"minecraft:diamond_sword\",tag:{display:{Name:\"Cool Sword\"}}}]] now";
        assert_eq!(field.handle_item_setting(tagged), "look Cool Sword now");
        assert!(field.has_stack());
        assert_eq!(field.get_insertion_start(), 5);
        // has_stack: passthrough
        assert_eq!(field.handle_item_setting("anything"), "anything");
        // stale link
        let mut field2 = ItemSharingTextFieldWidget::new();
        let stale = "[[smwyg:{not:nbt]] tail";
        assert_eq!(
            field2.handle_item_setting(stale),
            format!("{}{} tail", STALE_LINK_TEXT, "")
        );
        assert!(!field2.has_stack());
    }

    #[test]
    fn test_suggestion_offset_tracking() {
        let mut field = ItemSharingTextFieldWidget::new();
        field.set_stack(sword_stack(1));
        field.inserted_index = 10;
        field.on_suggestion_inserted(8, 4);
        assert_eq!(field.inserted_index, 14);
        field.on_suggestion_inserted(20, 4); // start > index: ignored
        assert_eq!(field.inserted_index, 14);
    }

    #[test]
    fn test_chat_send_message_flow() {
        let mut client = ClientState::default();
        let mut field = ItemSharingTextFieldWidget::new();
        assert!(matches!(
            chat_send_message(&field, "hi", true),
            SendMessageOutcome::Normal {
                add_to_history: true
            }
        ));
        client.sharing_item = Some(sword_stack(1));
        assert!(chat_screen_init(&mut client, &mut field));
        assert!(field.has_stack());
        assert!(client.sharing_item.is_none());
        field.inserted_index = 0;
        let text = "Cool Sword!";
        let out = chat_send_message(&field, text, true);
        match out {
            SendMessageOutcome::Shared {
                history_text,
                packet,
                ..
            } => {
                assert!(history_text.starts_with("[[smwyg:{"));
                assert!(history_text.ends_with("]]!"));
                let mut read = PacketBuf::from_bytes(packet.bytes);
                let p = read.read_packet().unwrap();
                assert_eq!(p.stack.id, "minecraft:diamond_sword");
            }
            _ => panic!("expected Shared"),
        }
        // history navigation resets
        chat_screen_set_chat_from_history(&mut field);
        assert!(!field.has_stack());
    }

    #[test]
    fn test_hide_stack_predicate() {
        let mut stack = sword_stack(1);
        assert!(!should_hide_stack(&stack));
        stack
            .tag
            .as_mut()
            .unwrap()
            .put("smwyg:hide_stack", NbtValue::Byte(1));
        assert!(should_hide_stack(&stack));
    }
}
