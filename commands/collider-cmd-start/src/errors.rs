use collider_common::{
    miette::{self, Diagnostic},
    thiserror::{self, Error},
};

#[derive(Debug, Error, Diagnostic)]
pub enum StartError {
    #[error(transparent)]
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
    GitHubApiError(octocrab::Error),

    #[error("{0}")]
    #[diagnostic(
        code(collider::start::github_api::request_limit),
        help("Consider passing in a GitHub API Token using `--github-token`, or using a different one."),
    )]
    GitHubApiLimit(octocrab::GitHubError),

    #[error("Could not find matching Electron files for release: {target}.")]
    #[diagnostic(code(collider::start::missing_electron_files))]
    MissingElectronFiles {
        version: node_semver::Version,
        target: String,
    },

    #[error("A matching electron version could not be found for `electron@{0}`")]
    #[diagnostic(code(collider::start::matching_version_not_found))]
    MatchingVersionNotFound(node_semver::Range),

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
    #[diagnostic(transparent)]
    SemverError(#[from] node_semver::SemverError),

    #[error("Failed to parse package.json")]
    #[diagnostic(code(collider::start::parse_package_json))]
    ParsePackageJson(#[from] collider_common::serde_json::Error),

    #[error("Electron process exited with an error")]
    #[diagnostic(code(collider::start::electron_error))]
    ElectronFailed,
}

impl From<octocrab::Error> for StartError {
    fn from(err: octocrab::Error) -> Self {
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
