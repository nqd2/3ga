pub mod editor_export;
mod job;
mod source;
mod webar;


pub use editor_export::{
    export_edited_source, EditorSourceExport, EditorSourceExportRequest,
};
pub use job::{cancel_job, process_job, ProcessRequest};
pub use source::{load_source, SourceMetadata};
pub use webar::{open_webar_viewer, save_bundle, SaveBundleRequest};

pub struct AppState {
    pub wgpu_ctx: Option<augmented_gaussian_core::gpu::WgpuContext>,
}

impl AppState {
    pub fn new(wgpu_ctx: Option<augmented_gaussian_core::gpu::WgpuContext>) -> Self {
        Self { wgpu_ctx }
    }
}
