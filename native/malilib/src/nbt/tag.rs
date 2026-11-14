use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum NbtTag {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(Vec<NbtTag>),
    Compound(HashMap<String, NbtTag>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl NbtTag {
    pub fn type_id(&self) -> u8 {
        match self {
            NbtTag::End => 0,
            NbtTag::Byte(_) => 1,
            NbtTag::Short(_) => 2,
            NbtTag::Int(_) => 3,
            NbtTag::Long(_) => 4,
            NbtTag::Float(_) => 5,
            NbtTag::Double(_) => 6,
            NbtTag::ByteArray(_) => 7,
            NbtTag::String(_) => 8,
            NbtTag::List(_) => 9,
            NbtTag::Compound(_) => 10,
            NbtTag::IntArray(_) => 11,
            NbtTag::LongArray(_) => 12,
        }
    }

    pub fn as_byte(&self) -> Option<i8> {
        match self {
            NbtTag::Byte(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i16(&self) -> Option<i16> {
        match self {
            NbtTag::Short(s) => Some(*s),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            NbtTag::Int(i) => Some(*i),
            NbtTag::Short(s) => Some(*s as i32),
            NbtTag::Byte(b) => Some(*b as i32),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            NbtTag::Long(l) => Some(*l),
            NbtTag::Int(i) => Some(*i as i64),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match self {
            NbtTag::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            NbtTag::Double(d) => Some(*d),
            NbtTag::Float(f) => Some(*f as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            NbtTag::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_compound(&self) -> Option<&HashMap<String, NbtTag>> {
        match self {
            NbtTag::Compound(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_compound_mut(&mut self) -> Option<&mut HashMap<String, NbtTag>> {
        match self {
            NbtTag::Compound(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[NbtTag]> {
        match self {
            NbtTag::List(list) => Some(list.as_slice()),
            _ => None,
        }
    }

    pub fn as_list_mut(&mut self) -> Option<&mut Vec<NbtTag>> {
        match self {
            NbtTag::List(list) => Some(list),
            _ => None,
        }
    }

    pub fn as_byte_array(&self) -> Option<&[i8]> {
        match self {
            NbtTag::ByteArray(arr) => Some(arr.as_slice()),
            _ => None,
        }
    }

    pub fn as_int_array(&self) -> Option<&[i32]> {
        match self {
            NbtTag::IntArray(arr) => Some(arr.as_slice()),
            _ => None,
        }
    }

    pub fn as_long_array(&self) -> Option<&[i64]> {
        match self {
            NbtTag::LongArray(arr) => Some(arr.as_slice()),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&NbtTag> {
        self.as_compound()?.get(key)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    pub fn get_i32(&self, key: &str) -> Option<i32> {
        self.get(key)?.as_i32()
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key)?.as_i64()
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key)?.as_f64()
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key)? {
            NbtTag::Byte(b) => Some(*b != 0),
            _ => None,
        }
    }

    pub fn get_compound(&self, key: &str) -> Option<&HashMap<String, NbtTag>> {
        self.get(key)?.as_compound()
    }

    pub fn get_list(&self, key: &str) -> Option<&[NbtTag]> {
        self.get(key)?.as_list()
    }
}

impl fmt::Display for NbtTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NbtTag::End => write!(f, "TAG_End"),
            NbtTag::Byte(v) => write!(f, "{}b", v),
            NbtTag::Short(v) => write!(f, "{}s", v),
            NbtTag::Int(v) => write!(f, "{}", v),
            NbtTag::Long(v) => write!(f, "{}L", v),
            NbtTag::Float(v) => write!(f, "{}f", v),
            NbtTag::Double(v) => write!(f, "{}d", v),
            NbtTag::ByteArray(v) => write!(f, "[B; {} bytes]", v.len()),
            NbtTag::String(v) => write!(f, "{:?}", v),
            NbtTag::List(v) => {
                write!(f, "[")?;
                for (i, item) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            NbtTag::Compound(v) => {
                write!(f, "{{")?;
                for (i, (key, val)) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", key, val)?;
                }
                write!(f, "}}")
            }
            NbtTag::IntArray(v) => write!(f, "[I; {} ints]", v.len()),
            NbtTag::LongArray(v) => write!(f, "[L; {} longs]", v.len()),
        }
    }
}
