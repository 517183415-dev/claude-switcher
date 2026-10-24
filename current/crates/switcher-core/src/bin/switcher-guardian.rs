#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    #[cfg(target_os = "macos")]
    if args == ["--probe-events"] {
        match switcher_core::guardian::native_events() {
            Ok((guard, receiver)) => {
                drop(receiver);
                drop(guard);
                println!(
                    "network_observer=registered_and_released\npower_observer=registered_and_released"
                );
                return;
            }
            Err(code) => {
                eprintln!("{code}");
                std::process::exit(1);
            }
        }
    }
    if !args.is_empty() {
        eprintln!("unsupported guardian argument");
        std::process::exit(2);
    }
    #[cfg(target_os = "macos")]
    {
        let uid = unsafe { libc::geteuid() };
        let record = match switcher_helper::platform::load_root_record(std::path::Path::new(
            switcher_domain::system::INSTALL_RECORD,
        )) {
            Ok(record) => record,
            Err(code) => {
                eprintln!("{code}");
                std::process::exit(1);
            }
        };
        // A global root-owned LaunchAgent runs in each session. Only its recorded owner remains active.
        if uid != record.owner_uid || uid == 0 {
            return;
        }
        let result = async {
            let home = switcher_core::config::home_dir()?;
            let mut config = switcher_core::config::load_read_only(&home).unwrap_or_else(|_| {
                // Unknown mode follows the old bodyguard's verge-mihomo fallback.
                switcher_core::config::CheckConfig {
                    proxy_mode: "clash-verge".into(),
                    ..Default::default()
                }
            });
            if !home.join(".claude-switcher/v2/config.json").exists()
                && !home.join(".local/bin/claude-switch.sh").exists()
            {
                config.proxy_mode = "clash-verge".into();
            }
            let directory = switcher_core::guardian::prepare_state_directory(&home)?;
            let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
            switcher_core::guardian::run_native(
                config,
                &directory.join("guardian-health.json"),
                shutdown_rx,
            )
            .await
        }
        .await;
        if let Err(code) = result {
            eprintln!("{code}");
            std::process::exit(1);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("macos_required");
        std::process::exit(1);
    }
}
