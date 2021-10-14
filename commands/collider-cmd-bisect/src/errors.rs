use collider_common::{
    miette::{self, Diagnostic},
    thiserror::{self, Error},
};

#[derive(Debug, Error, Diagnostic)]
pub enum BisectError {
    #[error(transparent)]
    #[diagnostic(code(collider::bisect::http_error))]
    HttpError(#[from] reqwest::Error),

    #[error(transparent)]
    #[diagnostic(code(collider::bisect::io_error))]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    #[diagnostic(code(collider::bisect::semver_error))]
    SemverError(#[from] node_semver::SemverError),

    #[error("Electron process exited with an error")]
    #[diagnostic(code(collider::bisect::electron_error))]
    ElectronFailed,
}
