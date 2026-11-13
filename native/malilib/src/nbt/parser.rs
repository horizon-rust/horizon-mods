use std::collections::HashMap;
use std::io::{self, Cursor, Read};
use byteorder::{BigEndian, ReadBytesExt};
use flate2::read::{GzDecoder, ZlibDecoder};
use super::tag::NbtTag;

pub struct NbtParser;

impl NbtParser {
    pub fn read_compressed<R: Read>(mut reader: R) -> io::Result<(String, NbtTag)> {
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;

        if buffer.len() >= 2 && buffer[0] == 0x1f && buffer[1] == 0x8b {
            let mut gz = GzDecoder::new(Cursor::new(buffer));
            Self::read(&mut gz)
        } else if buffer.len() >= 2 && buffer[0] == 0x78 && (buffer[1] == 0x01 || buffer[1] == 0x9c || buffer[1] == 0xda) {
            let mut zlib = ZlibDecoder::new(Cursor::new(buffer));
            Self::read(&mut zlib)
        } else {
            let mut cursor = Cursor::new(buffer);
            Self::read(&mut cursor)
        }
    }

    pub fn read<R: Read>(reader: &mut R) -> io::Result<(String, NbtTag)> {
        let type_id = reader.read_u8()?;
        if type_id == 0 {
            return Ok(("".to_string(), NbtTag::End));
        }
        let name = Self::read_string(reader)?;
        let tag = Self::read_payload(type_id, reader)?;
        Ok((name, tag))
    }

    pub fn read_payload<R: Read>(type_id: u8, reader: &mut R) -> io::Result<NbtTag> {
        match type_id {
            0 => Ok(NbtTag::End),
            1 => Ok(NbtTag::Byte(reader.read_i8()?)),
            2 => Ok(NbtTag::Short(reader.read_i16::<BigEndian>()?)),
            3 => Ok(NbtTag::Int(reader.read_i32::<BigEndian>()?)),
            4 => Ok(NbtTag::Long(reader.read_i64::<BigEndian>()?)),
            5 => Ok(NbtTag::Float(reader.read_f32::<BigEndian>()?)),
            6 => Ok(NbtTag::Double(reader.read_f64::<BigEndian>()?)),
            7 => {
                let len = reader.read_i32::<BigEndian>()?;
                if len < 0 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Negative ByteArray length"));
                }
                let mut buf = vec![0i8; len as usize];
                let slice = unsafe { std::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, len as usize) };
                reader.read_exact(slice)?;
                Ok(NbtTag::ByteArray(buf))
            }
            8 => Ok(NbtTag::String(Self::read_string(reader)?)),
            9 => {
                let elem_type = reader.read_u8()?;
                let len = reader.read_i32::<BigEndian>()?;
                if len <= 0 {
                    return Ok(NbtTag::List(Vec::new()));
                }
                let mut list = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    list.push(Self::read_payload(elem_type, reader)?);
                }
                Ok(NbtTag::List(list))
            }
            10 => {
                let mut map = HashMap::new();
                loop {
                    let field_type = reader.read_u8()?;
                    if field_type == 0 {
                        break;
                    }
                    let field_name = Self::read_string(reader)?;
                    let field_tag = Self::read_payload(field_type, reader)?;
                    map.insert(field_name, field_tag);
                }
                Ok(NbtTag::Compound(map))
            }
            11 => {
                let len = reader.read_i32::<BigEndian>()?;
                if len < 0 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Negative IntArray length"));
                }
                let mut list = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    list.push(reader.read_i32::<BigEndian>()?);
                }
                Ok(NbtTag::IntArray(list))
            }
            12 => {
                let len = reader.read_i32::<BigEndian>()?;
                if len < 0 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Negative LongArray length"));
                }
                let mut list = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    list.push(reader.read_i64::<BigEndian>()?);
                }
                Ok(NbtTag::LongArray(list))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown NBT tag type: {}", type_id),
            )),
        }
    }

    fn read_string<R: Read>(reader: &mut R) -> io::Result<String> {
        let len = reader.read_u16::<BigEndian>()? as usize;
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}
