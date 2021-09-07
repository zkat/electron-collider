use collider_common::{
    miette::{self, Diagnostic},
    thiserror::{self, Error},
};

#[derive(Debug, Error, Diagnostic)]
pub enum StartError {
    #[error("{0}")]
    #[diagnostic(code(collider::start::http_error))]
    HttpError(#[from] reqwest::Error),

    #[error(transparent)]
    #[diagnostic(code(collider::start::io_error))]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    #[diagnostic(code(collider::start::zip_error))]
    ZipError(#[from] zip::result::ZipError),

    #[error(transparent)]
    #[diagnostic(code(collider::start::github_api))]
    GitHubApiError(#[from] octocrab::Error),

    #[error("{0}")]
    #[diagnostic(
        code(collider::start::github_api::request_limit),
        help("Consider passing in a GitHub API Token using `--github-token`, or using a different one."),
    )]
    GitHubApiLimit(octocrab::GitHubError),

    #[error("Could not find matching Electron files for release: {target}.")]
    #[diagnostic(code(collider::start::missing_electron_files))]
    MissingElectronFiles {
        version: collider_node_semver::Version,
        target: String,
    },

    #[error("A matching electron version could not be found for `electron@{0}`")]
    #[diagnostic(code(collider::start::matching_version_not_found))]
    MatchingVersionNotFound(collider_node_semver::Range),

    #[error("Unsupported architecture: {0}.")]
    #[diagnostic(
        code(collider::start::unsupported_arch),
        help("Electron only supports ia32, x64, arm64, and arm7l.")
    )]
    UnsupportedArch(String),

    #[error("Unsupported platform: {0}.")]
    #[diagnostic(
        code(collider::start::unsupported_arch),
        help("Electron only supports win32, linux, and darwin.")
    )]
    UnsupportedPlatform(String),

    #[error("Platform-specific project directory could not be determined.")]
    #[diagnostic(code(collider::start::no_project_dir))]
    NoProjectDir,

    #[error(transparent)]
    #[diagnostic(code(collider::start::semver_error))]
    SemverError(#[from] collider_node_semver::SemverError),
}

impl StartError {
    pub(crate) fn from_octocrab(err: octocrab::Error) -> Self {
        match err {
            octocrab::Error::GitHub {
                source: ref gh_err, ..
            } if gh_err.message.contains("rate limit exceeded") => {
                StartError::GitHubApiLimit(gh_err.clone())
            }
            _ => StartError::GitHubApiError(err),
        }
    }
}
