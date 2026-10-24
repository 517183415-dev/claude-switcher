fn main() {
    #[cfg(target_os = "macos")]
    {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args == ["--probe"] {
            let (self_valid, apple_valid, forged_valid) = switcher_helper::xpc::signature_probe();
            println!(
                "self_signature_valid={self_valid}\napple_identity_valid={apple_valid}\nsynthetic_xpc_authenticated={forged_valid}"
            );
            return;
        }
        if args.len() == 2 && args[0] == "--root-operation" {
            if args[1].len() > 16384 {
                std::process::exit(2);
            }
            let result = serde_json::from_str(&args[1])
                .map_err(|_| "root_operation_encoding")
                .and_then(switcher_helper::root_operation::execute);
            if let Err(code) = result {
                eprintln!("{code}");
                std::process::exit(1);
            }
            return;
        }
        if !args.is_empty() {
            eprintln!("helper accepts only launchd activation or --probe");
            std::process::exit(2);
        }
        let result = switcher_helper::platform::load_root_record(std::path::Path::new(
            switcher_domain::system::INSTALL_RECORD,
        ))
        .and_then(|record| {
            if !switcher_helper::root_operation::source_local_pending()? {
                let _root_operation = switcher_helper::root_operation::RootLock::acquire()?;
                switcher_helper::platform::recover_pending_repair()?;
            }
            switcher_helper::xpc::serve(record)
        });
        if let Err(code) = result {
            eprintln!("helper: {code}");
            std::process::exit(1);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("helper requires macOS");
        std::process::exit(1);
    }
}
