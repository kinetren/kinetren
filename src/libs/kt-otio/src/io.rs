//! Чтение/запись .otio-документа.
//!
//! Запись детерминирована: `OTIO_SCHEMA` первым ключом, известные поля в
//! порядке объявления структур, passthrough — в порядке исходного документа;
//! отступ 4 пробела (стиль эталонной библиотеки), завершающий перевод строки
//! (домашний стиль). Идемпотентность `write(parse(write(x))) == write(x)`
//! закреплена тестом 144.

use serde::Serialize;
use serde_json::Value;

use crate::schema::{decode_timeline, encode_timeline};
use crate::{Error, Timeline};

pub fn parse_document(text: &str) -> Result<Timeline, Error> {
    let v: Value = serde_json::from_str(text)?;
    decode_timeline(v)
}

pub fn write_document(t: &Timeline) -> String {
    let v = encode_timeline(t);
    let mut out = Vec::with_capacity(16 * 1024);
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut out, fmt);
    v.serialize(&mut ser).expect("Value serialization is infallible");
    out.push(b'\n');
    String::from_utf8(out).expect("serde_json emits valid UTF-8")
}
