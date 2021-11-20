fn main() {
    // Add build time to binary, by setting an environment variable for rustc.
    // See https://stackoverflow.com/questions/43753491/include-git-commit-hash-as-string-into-rust-program
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", chrono::Local::now().to_rfc2822());

    // Do not constantly rebuild, just because the BUILD_TIMESTAMP changed.
    // FIXME this causes the BUILD_TIMESTAMP to not always update, even when there are changes!?
    // println!("cargo:rerun-if-env-changed=")
}
