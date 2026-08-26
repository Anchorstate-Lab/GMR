#[cfg(any(feature = "git", feature = "claude-code"))]
mod local_file;

#[cfg(feature = "http")]
mod http;

#[cfg(feature = "declared")]
pub mod declared;

#[cfg(feature = "git")]
pub mod git;

#[cfg(feature = "claude-code")]
pub mod claude_code;

#[cfg(feature = "mem0")]
pub mod mem0;

#[cfg(any(feature = "git", feature = "claude-code"))]
pub(crate) fn spend(budget: &gmr_budget::Budget) -> Result<(), gmr_content::ContentError> {
    budget
        .checkpoint()
        .map_err(|s| gmr_content::ContentError::spent(s.as_str()))
}

#[cfg(feature = "git")]
pub(crate) async fn within(
    command: std::process::Command,
    budget: &gmr_budget::Budget,
) -> Result<std::process::Output, gmr_content::ContentError> {
    spend(budget)?;
    let left = budget
        .remaining()
        .ok_or_else(|| gmr_content::ContentError::spent("no time left to run git"))?;

    let mut command = tokio::process::Command::from(command);
    command
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);

    tokio::time::timeout(left, command.output())
        .await
        .map_err(|_| {
            gmr_content::ContentError::spent(
                "git was still running when this call's budget ran out, so it was killed. A \
                 deadline that only decides whether to start is not a deadline",
            )
        })?
        .map_err(|e| gmr_content::ContentError::new(format!("cannot run git: {e}")))
}
