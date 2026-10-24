fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/desktop/src/generated")
        });
    switcher_domain::export_types(&target)?;
    println!("Exported domain types to {}", target.display());
    Ok(())
}
