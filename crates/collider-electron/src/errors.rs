use std::cmp;

use collider_common::{
    miette::{self, Diagnostic, NamedSource, SourceOffset},
    thiserror::{self, Error},
};

#[derive(Debug, Error, Diagnostic)]
pub enum ElectronError {
    #[error(transparent)]
    #[diagnostic(code(collider::electron::http_error))]
    HttpError(#[from] reqwest::Error),

    #[error("{0}")]
    #[diagnostic(code(collider::electron::io_error))]
    IoError(String, #[source] std::io::Error),

    #[error(transparent)]
    #[diagnostic(code(collider::electron::copy_error))]
    FsExtraError(#[from] fs_extra::error::Error),

    #[error("Failed to get the currently-executing collider binary")]
    #[diagnostic(
        code(collider::electron::current_exe_failure),
        help("Acquiring the path of the current executable is a platform-specific operation that can fail for a good number of reasons. Some errors can include, but not be limited to, filesystem operations failing or general syscall failures.")
    )]
    CurrentExeFailure(#[source] std::io::Error),

    #[error("Found some bad JSON")]
    #[diagnostic(code(collider::electron::bad_package_json))]
    BadJson {
        source: collider_common::serde_json::Error,
        url: String,
        #[source_code]
        json: NamedSource,
        #[label("here")]
        err_loc: (usize, usize),
    },

    #[error(transparent)]
    #[diagnostic(code(collider::electron::zip_error))]
    ZipError(#[from] zip::result::ZipError),

    #[error(transparent)]
    #[diagnostic(code(collider::electron::github_api))]
    GitHubApiError(octocrab::Error),

    #[error("{0}")]
    #[diagnostic(
        code(collider::electron::github_api::request_limit),
        help("Consider passing in a GitHub API Token using `--github-token`, or using a different one."),
    )]
    GitHubApiLimit(octocrab::GitHubError),

    #[error("Could not find matching Electron files for release: {target}.")]
    #[diagnostic(code(collider::electron::missing_electron_files))]
    MissingElectronFiles {
        version: node_semver::Version,
        target: String,
    },

    #[error("A matching electron version could not be found for `electron@{0}`")]
    #[diagnostic(code(collider::electron::matching_version_not_found))]
    MatchingVersionNotFound(node_semver::Range),

    #[error("Unsupported architecture: {0}.")]
    #[diagnostic(
        code(collider::electron::unsupported_arch),
        help("Electron only supports ia32, x64, arm64, and arm7l.")
    )]
    UnsupportedArch(String),

    #[error("Unsupported platform: {0}.")]
    #[diagnostic(
        code(collider::electron::unsupported_arch),
        help("Electron only supports win32, linux, and darwin.")
    )]
    UnsupportedPlatform(String),

    #[error("Platform-specific project directory could not be determined.")]
    #[diagnostic(code(collider::electron::no_project_dir))]
    NoProjectDir,

    #[error(transparent)]
    #[diagnostic(code(collider::electron::semver_error))]
    SemverError(#[from] node_semver::SemverError),

    #[error("Failed to parse package.json")]
    #[diagnostic(code(collider::electron::parse_package_json))]
    ParsePackageJson(#[from] collider_common::serde_json::Error),

    #[error("Electron process exited with an error")]
    #[diagnostic(code(collider::electron::electron_error))]
    ElectronFailed,
}

impl From<octocrab::Error> for ElectronError {
    fn from(err: octocrab::Error) -> Self {
        match err {
            octocrab::Error::GitHub {
                source: ref gh_err, ..
            } if gh_err.message.contains("rate limit exceeded") => {
                ElectronError::GitHubApiLimit(gh_err.clone())
            }
            _ => ElectronError::GitHubApiError(err),
        }
    }
}

impl ElectronError {
    pub fn from_json_err(
        err: collider_common::serde_json::Error,
        url: String,
        json: String,
    ) -> Self {
        // These json strings can get VERY LONG and miette doesn't (yet?)
        // support any "windowing" mechanism for displaying stuff, so we have
        // to manually shorten the string to only the relevant bits and
        // translate the spans accordingly.
        let err_offset = SourceOffset::from_location(&json, err.line(), err.column());
        let json_len = json.len();
        let local_offset = err_offset.offset().saturating_sub(40);
        let local_len = cmp::min(40, json_len - err_offset.offset());
        let snipped_json = json[local_offset..err_offset.offset() + local_len].to_string();
        Self::BadJson {
            source: err,
            url: url.clone(),
            json: NamedSource::new(url, snipped_json),
            err_loc: (err_offset.offset() - local_offset, 0),
        }
    }
}
