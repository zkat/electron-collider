use collider_common::{
    miette::{self, Diagnostic},
    thiserror::{self, Error},
};

#[derive(Debug, Error, Diagnostic)]
pub enum StartError {
    #[error(transparent)]
    #[diagnostic(code(collider::start::io_error))]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    #[diagnostic(code(collider::start::semver_error))]
    SemverError(#[from] node_semver::SemverError),

    #[error("Electron process exited with an error")]
    #[diagnostic(code(collider::start::electron_error))]
    ElectronFailed,
}
