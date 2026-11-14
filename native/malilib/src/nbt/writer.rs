use std::io::{self, Write};
use byteorder::{BigEndian, WriteBytesExt};
use flate2::write::GzEncoder;
use flate2::Compression;
use super::tag::NbtTag;

pub struct NbtWriter;

impl NbtWriter {
    pub fn write_compressed<W: Write>(name: &str, tag: &NbtTag, writer: W) -> io::Result<()> {
        let mut gz = GzEncoder::new(writer, Compression::default());
        Self::write(name, tag, &mut gz)?;
        gz.finish()?;
        Ok(())
    }

    pub fn write<W: Write>(name: &str, tag: &NbtTag, writer: &mut W) -> io::Result<()> {
        let type_id = tag.type_id();
        writer.write_u8(type_id)?;
        if type_id == 0 {
            return Ok(());
        }
        Self::write_string(name, writer)?;
        Self::write_payload(tag, writer)?;
        Ok(())
    }

    pub fn write_payload<W: Write>(tag: &NbtTag, writer: &mut W) -> io::Result<()> {
        match tag {
            NbtTag::End => Ok(()),
            NbtTag::Byte(v) => writer.write_i8(*v),
            NbtTag::Short(v) => writer.write_i16::<BigEndian>(*v),
            NbtTag::Int(v) => writer.write_i32::<BigEndian>(*v),
            NbtTag::Long(v) => writer.write_i64::<BigEndian>(*v),
            NbtTag::Float(v) => writer.write_f32::<BigEndian>(*v),
            NbtTag::Double(v) => writer.write_f64::<BigEndian>(*v),
            NbtTag::ByteArray(arr) => {
                writer.write_i32::<BigEndian>(arr.len() as i32)?;
                let slice = unsafe { std::slice::from_raw_parts(arr.as_ptr() as *const u8, arr.len()) };
                writer.write_all(slice)
            }
            NbtTag::String(s) => Self::write_string(s, writer),
            NbtTag::List(list) => {
                let elem_type = list.first().map(|t| t.type_id()).unwrap_or(0);
                writer.write_u8(elem_type)?;
                writer.write_i32::<BigEndian>(list.len() as i32)?;
                for item in list {
                    Self::write_payload(item, writer)?;
                }
                Ok(())
            }
            NbtTag::Compound(map) => {
                for (key, val) in map {
                    writer.write_u8(val.type_id())?;
                    Self::write_string(key, writer)?;
                    Self::write_payload(val, writer)?;
                }
                writer.write_u8(0) // TAG_End
            }
            NbtTag::IntArray(arr) => {
                writer.write_i32::<BigEndian>(arr.len() as i32)?;
                for &v in arr {
                    writer.write_i32::<BigEndian>(v)?;
                }
                Ok(())
            }
            NbtTag::LongArray(arr) => {
                writer.write_i32::<BigEndian>(arr.len() as i32)?;
                for &v in arr {
                    writer.write_i64::<BigEndian>(v)?;
                }
                Ok(())
            }
        }
    }

    fn write_string<W: Write>(s: &str, writer: &mut W) -> io::Result<()> {
        let bytes = s.as_bytes();
        writer.write_u16::<BigEndian>(bytes.len() as u16)?;
        writer.write_all(bytes)
    }
}
