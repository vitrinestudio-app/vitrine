fn main() {
    // `generate_context!` embeds the icons at compile time; rebuild when they change.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build();
}
