pub mod build;
pub mod db;
pub mod extractors;
pub mod queries;

pub const SCHEMA: &str = include_str!("schema.sql");
