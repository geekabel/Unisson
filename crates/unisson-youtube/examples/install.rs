//! Downloads the YouTube component into a folder, as Unisson does at the
//! first search, then asks the downloaded yt-dlp for its version.
//!
//! ```text
//! cargo run -p unisson-youtube --example install -- /tmp/unisson-youtube
//! ```

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use unisson_youtube::{Component, Platform, Status, Web, YtDlp};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir: PathBuf = std::env::args_os()
        .nth(1)
        .ok_or("give the folder to download into")?
        .into();
    let platform = Platform::current().ok_or("no YouTube component for this system")?;
    let tool = YtDlp::new();
    let component = Component::new(dir, platform, tool.clone(), Box::new(Web::new()));

    let (sender, receiver) = mpsc::channel();
    let sender = std::sync::Mutex::new(sender);
    component.watch(move |status| {
        let _ = sender.lock().map(|sender| sender.send(status.clone()));
    });
    component.prepare();
    if component.status() != Status::Ready {
        loop {
            match receiver.recv_timeout(Duration::from_secs(20 * 60))? {
                Status::Installing { percent } => {
                    if let Some(percent) = percent {
                        eprint!("\r{percent:3} %");
                    }
                }
                Status::Ready => break,
                Status::Failed(reason) => return Err(reason.into()),
                Status::Absent => {}
            }
        }
        eprintln!();
    }
    println!("{} {}", tool.program().display(), tool.version()?);
    Ok(())
}
