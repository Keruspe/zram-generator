/* SPDX-License-Identifier: MIT */

use crate::config::Device;
use anyhow::{anyhow, Context, Result};
use log::warn;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::Command;

const SYSTEMD_MAKEFS_COMMAND: &str = concat!(
    env!(
        "SYSTEMD_UTIL_DIR",
        "Define $SYSTEMD_UTIL_DIR to the result of \
         $(pkg-config --variable=systemdutildir systemd) (e.g. /usr/lib/systemd/)"
    ),
    "/systemd-makefs"
);

pub fn run_device_setup(device: Option<Device>, device_name: &str) -> Result<()> {
    let device = device.ok_or_else(|| anyhow!("Device {} not found", device_name))?;

    let device_sysfs_path = Path::new("/sys/block").join(device_name);

    if let Some(compression_algorithm) = device.compression_algorithm {
        let comp_algorithm_path = device_sysfs_path.join("comp_algorithm");
        match fs::write(&comp_algorithm_path, &compression_algorithm) {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::InvalidInput => {
                warn!(
                    "Warning: algorithm {:?} not recognised; consult {} for a list of available ones",
                    compression_algorithm, comp_algorithm_path.display(),
                );
            }
            err @ Err(_) => err.with_context(|| {
                format!(
                    "Failed to configure compression algorithm into {}",
                    comp_algorithm_path.display()
                )
            })?,
        }
    }

    let disksize_path = device_sysfs_path.join("disksize");
    fs::write(&disksize_path, format!("{}", device.disksize)).with_context(|| {
        format!(
            "Failed to configure disk size into {}",
            disksize_path.display()
        )
    })?;

    match Command::new(SYSTEMD_MAKEFS_COMMAND).arg("swap").arg(Path::new("/dev").join(device_name)).status() {
        Ok(status) =>
            match status.code() {
                Some(0) => Ok(()),
                Some(code) => Err(anyhow!("{} failed with exit code {}", SYSTEMD_MAKEFS_COMMAND, code)),
                None => Err(anyhow!("{} terminated by signal {}",
                                    SYSTEMD_MAKEFS_COMMAND,
                                    status.signal().expect("on unix, status status.code() is None iff status.signal() isn't; \
                                                            this expect() will never panic, save for an stdlib bug"))),
            },
        Err(e) =>
            Err(e).with_context(|| {
                format!(
                    "{} call failed for /dev/{}",
                    SYSTEMD_MAKEFS_COMMAND,
                    device_name
                )
            }),
    }
}

pub fn run_device_reset(device_name: &str) -> Result<()> {
    let reset = Path::new("/sys/block").join(device_name).join("reset");
    fs::write(reset, b"1")?;
    Ok(())
}
