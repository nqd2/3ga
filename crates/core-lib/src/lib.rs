pub mod config;
pub mod error;
pub mod evaluation;
pub mod gpu;
pub mod math;
pub mod pipeline;
pub mod splat_table;

pub mod output;
pub mod processing;
pub mod readers;

// Re-export nested sub-modules flatly to preserve API backward-compatibility
pub use output::{glb, manifest, webar};
pub use processing::{alignment, filters, mesh, navmesh, voxel};

pub use config::{ProcessConfig, RecipeBundle};
pub use error::{AgError, AgResult};
pub use manifest::Manifest;
pub use pipeline::{ProcessOutput, process_file};
pub use splat_table::SplatTable;
