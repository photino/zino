//! Extension traits to provide helper utilities.

mod avro_record;
mod header_map;
mod json_object;
mod toml_table;

pub use avro_record::AvroRecordExt;
pub use header_map::HeaderMapExt;
pub use json_object::JsonObjectExt;
pub use toml_table::TomlTableExt;
