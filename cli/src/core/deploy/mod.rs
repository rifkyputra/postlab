pub mod detector;
pub mod git;
pub mod runner;

pub use detector::detect_deployment_type;
pub use git::{clone_repo, pull_repo};
pub use runner::{start_deployment, stop_deployment};
