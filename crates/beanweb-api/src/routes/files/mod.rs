//! File editor routes - File listing and editing

pub mod api;
pub mod page;

pub use api::{api_files_list, api_file_content, api_file_save};
pub use page::{page_files, page_file_edit};
