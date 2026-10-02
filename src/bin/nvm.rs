fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(nvmrc::cli::run_from_env())
}
