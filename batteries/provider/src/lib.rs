#[cfg(any(feature = "git", feature = "claude-code"))]
mod local_file;

#[cfg(feature = "git")]
pub mod git;

#[cfg(feature = "claude-code")]
pub mod claude_code;

#[cfg(feature = "mem0")]
pub mod mem0;

#[cfg(any(feature = "git", feature = "claude-code"))]
pub(crate) fn spend(budget: &gmr_probe::Budget) -> Result<(), gmr_content::ContentError> {
    budget
        .checkpoint()
        .map_err(|s| gmr_content::ContentError::spent(s.as_str()))
}

#[cfg(feature = "git")]
pub(crate) fn within(
    mut command: std::process::Command,
    budget: &gmr_probe::Budget,
) -> Result<std::process::Output, gmr_content::ContentError> {
    use std::io::Read;

    spend(budget)?;
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| gmr_content::ContentError::new(format!("cannot run git: {e}")))?;

    loop {
        match child.try_wait() {
            Err(e) => {
                return Err(gmr_content::ContentError::new(format!(
                    "cannot wait for git: {e}"
                )));
            }
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut pipe) = child.stdout.take() {
                    let _ = pipe.read_to_end(&mut stdout);
                }
                if let Some(mut pipe) = child.stderr.take() {
                    let _ = pipe.read_to_end(&mut stderr);
                }
                return Ok(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {}
        }
        if spend(budget).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(gmr_content::ContentError::spent(
                "git was still running when this call's budget ran out, so it was killed. A \
                 deadline that only decides whether to start is not a deadline",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
