pub mod tag;
pub mod parser;
pub mod writer;

pub use tag::NbtTag;
pub use parser::NbtParser;
pub use writer::NbtWriter;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Cursor;

    #[test]
    fn test_nbt_roundtrip() {
        let mut root = HashMap::new();
        root.insert("byteVal".to_string(), NbtTag::Byte(42));
        root.insert("shortVal".to_string(), NbtTag::Short(1000));
        root.insert("intVal".to_string(), NbtTag::Int(123456));
        root.insert("longVal".to_string(), NbtTag::Long(9876543210));
        root.insert("floatVal".to_string(), NbtTag::Float(3.14));
        root.insert("doubleVal".to_string(), NbtTag::Double(2.718281828));
        root.insert("stringVal".to_string(), NbtTag::String("Hello Horizon".to_string()));
        root.insert("byteArray".to_string(), NbtTag::ByteArray(vec![1, 2, 3, -1, -2]));
        root.insert("intArray".to_string(), NbtTag::IntArray(vec![100, 200, -300]));
        root.insert("longArray".to_string(), NbtTag::LongArray(vec![10000000000, -20000000000]));
        root.insert("listVal".to_string(), NbtTag::List(vec![
            NbtTag::String("item1".to_string()),
            NbtTag::String("item2".to_string()),
        ]));

        let mut sub = HashMap::new();
        sub.insert("nestedKey".to_string(), NbtTag::String("nestedValue".to_string()));
        root.insert("subCompound".to_string(), NbtTag::Compound(sub));

        let compound = NbtTag::Compound(root);

        let mut out = Vec::new();
        NbtWriter::write_compressed("RootTag", &compound, &mut out).unwrap();

        let (name, decoded) = NbtParser::read_compressed(Cursor::new(&out)).unwrap();
        assert_eq!(name, "RootTag");
        assert_eq!(decoded.get_i32("intVal"), Some(123456));
        assert_eq!(decoded.get_str("stringVal"), Some("Hello Horizon"));
        assert_eq!(decoded.get_i64("longVal"), Some(9876543210));
        assert_eq!(decoded.get("subCompound").unwrap().get_str("nestedKey"), Some("nestedValue"));
    }
}
