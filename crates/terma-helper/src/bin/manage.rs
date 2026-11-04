fn main() {
    std::process::exit(terma_helper::run_manage(std::env::args().skip(1).collect()));
}
