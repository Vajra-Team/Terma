fn main() {
    std::process::exit(terma_helper::run_aur(std::env::args().skip(1).collect()));
}
