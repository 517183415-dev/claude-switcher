fn main() {
    if switcher_core::ad_runtime::validate_public_release_materials().is_err() {
        eprintln!("Formal Public build requires valid production advertisement trust materials.");
        std::process::exit(1);
    }
}
