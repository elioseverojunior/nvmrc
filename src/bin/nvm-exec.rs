fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(nvmrc::cli::nvm_exec_from_env())
}
