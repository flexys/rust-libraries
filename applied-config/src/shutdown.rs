use actix_web::dev::ServerHandle;
use anyhow::Result;
use flexys_observability::category::APPLIED_CONFIG_LOADING;
use flexys_observability::layer::PLATFORM;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::task::JoinHandle;
use tokio_schedule::{every, Job};
use tracing::{error, info, warn};

pub async fn shutdown_on_config_change(
    polling_period_in_sec: u32,
    applied_config_folder: Box<PathBuf>,
    applied_config_last_modified: u64,
    server_handle: ServerHandle,
) -> JoinHandle<()> {
    let poll_applied_config_task = every(polling_period_in_sec).seconds().perform(move || {
        let handle_clone = server_handle.clone();
        let value = applied_config_folder.clone();

        async move {
            let folder_last_modified: Result<u64> =
                last_modified_as_seconds_since_epoch(value.as_ref());

            match folder_last_modified {
                Ok(last_modified) => {
                    if last_modified > applied_config_last_modified {
                        handle_clone.stop(true).await;
                    }
                }
                Err(_) => {
                    warn!(layer = PLATFORM,
                        category = APPLIED_CONFIG_LOADING,
                        "Unable to calculate folder last modified. Check for changed applied config will be done on next interval of {polling_period_in_sec} seconds")
                }
            }
        }
    });

    tokio::spawn(poll_applied_config_task)
}

fn last_modified_as_seconds_since_epoch(path: &Path) -> Result<u64> {
    let metadata = path.metadata()?;
    let modified = metadata.modified()?;
    let duration = modified.duration_since(SystemTime::UNIX_EPOCH)?;

    Ok(duration.as_secs())
}
