//! Explicit read-only metadata probe. It never reads Keychain secret data or account files.
fn main() {
    let report = switcher_core::migration::KeychainMetadataProbe::run();
    match serde_json::to_string_pretty(&report) {
        Ok(json) => println!("{json}"),
        Err(_) => {
            eprintln!("Keychain 属性调查结果无法编码");
            std::process::exit(2);
        }
    }
}
