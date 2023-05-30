fn main() {
    pkg_config::probe_library("libcec").unwrap();
}
