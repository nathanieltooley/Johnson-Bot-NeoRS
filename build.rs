fn main() {
    println!("cargo::rerun-if-changed=./.git/refs/heads/main");
    built::write_built_file().expect("should be able to get build info");
}
