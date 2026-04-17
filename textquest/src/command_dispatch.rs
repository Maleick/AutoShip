use anyhow::Result;

/// Dispatch a slash command to a local client without box-chat interception.
///
/// This path is reused by direct TUI/CLI sends and by the box-chat runtime when
/// a network message needs to execute on a local DLL client.
#[cfg(windows)]
pub fn dispatch_local_command(pid: u32, command: &str) -> Result<()> {
    use anyhow::Context;
    use textquest_common::ipc::Command;

    if let Some(message) = crate::nav::try_handle_local_slash_command(pid, command)? {
        tracing::info!(pid, %message, "Handled local slash command");
        return Ok(());
    }

    let token = crate::ipc::load_session_token(pid).with_context(|| {
        format!("missing session token for PID {pid}; inject the DLL before sending commands")
    })?;
    let session_id = textquest_common::ipc::session_id_from_token(&token);
    let pipe = crate::ipc::pipe::CommandPipe::connect(pid, session_id)?;
    pipe.send_raw_token(&token)?;
    pipe.send_async(&Command::SlashCommand {
        command: command.to_string(),
    })?;
    Ok(())
}

#[cfg(not(windows))]
pub fn dispatch_local_command(_pid: u32, _command: &str) -> Result<()> {
    anyhow::bail!("local slash command dispatch is only available on Windows builds")
}
