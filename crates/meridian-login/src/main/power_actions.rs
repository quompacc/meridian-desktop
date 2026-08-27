#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PowerCommand {
    program: &'static str,
    args: &'static [&'static str],
}

#[cfg(any(target_os = "openbsd", target_os = "freebsd"))]
fn power_command(action: PowerAction) -> std::io::Result<PowerCommand> {
    let args = match action {
        PowerAction::PowerOff => &["-p", "now"][..],
        PowerAction::Reboot => &["-r", "now"][..],
    };
    Ok(PowerCommand {
        program: "/sbin/shutdown",
        args,
    })
}

#[cfg(target_os = "linux")]
fn power_command(action: PowerAction) -> std::io::Result<PowerCommand> {
    let args = match action {
        PowerAction::PowerOff => &["poweroff"][..],
        PowerAction::Reboot => &["reboot"][..],
    };
    Ok(PowerCommand {
        program: "/usr/bin/systemctl",
        args,
    })
}

#[cfg(not(any(target_os = "openbsd", target_os = "freebsd", target_os = "linux")))]
fn power_command(_action: PowerAction) -> std::io::Result<PowerCommand> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "system power actions are unsupported on this platform",
    ))
}

fn run_power_action(action: PowerAction) -> std::io::Result<()> {
    let command = power_command(action)?;
    info!(
        action = action.label(),
        program = command.program,
        "requesting system power action"
    );
    let status = Command::new(command.program).args(command.args).status()?;
    if status.success() {
        info!(action = action.label(), "system power action accepted");
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "{} exited with {status}",
            command.program
        )))
    }
}

fn restore_greeter_after_power_result(
    action: PowerAction,
    result: &std::io::Result<()>,
) -> bool {
    if let Err(err) = result {
        warn!(
            action = action.label(),
            error = %err,
            "system power action failed; restoring greeter"
        );
        true
    } else {
        false
    }
}
