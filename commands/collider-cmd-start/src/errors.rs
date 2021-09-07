use collider_common::{
    miette::{self, Diagnostic},
    thiserror::{self, Error},
};

#[derive(Debug, Error, Diagnostic)]
pub enum StartError {
    #[error(transparent)]
    #[diagnostic(code(collider::start::github_api))]
    GitHubApiError(#[from] octocrab::Error),
    #[error("{0}")]
    #[diagnostic(
        code(collider::start::github_api::request_limit),
        help("Consider passing in a GitHub API Token using `--github-token`, or using a different one."),
    )]
    GitHubApiLimit(octocrab::GitHubError),
}
