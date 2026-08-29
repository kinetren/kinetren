//! Ключи каталога — генерируются `build.rs` из `i18n/en.toml`.
//! Здесь только подключение: смотреть содержимое — в `$OUT_DIR/i18n_keys.rs`.
include!(concat!(env!("OUT_DIR"), "/i18n_keys.rs"));
