pub mod image_to_base64;
pub mod json_compare;
pub mod json_formatter;
pub mod tsv_to_sql;
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolId {
    TsvToSql,
    ImageToBase64,
    JsonFormatter,
    JsonCompare,
    Settings,
}
