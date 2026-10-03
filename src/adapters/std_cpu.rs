use crate::ports::Cpu;

/// The real [`Cpu`]: the parallelism the operating system grants this
/// process (which respects container limits, unlike `/proc/cpuinfo`).
pub struct StdCpu;

impl Cpu for StdCpu {
    fn cores(&self) -> Option<usize> {
        std::thread::available_parallelism().ok().map(usize::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_machine_has_at_least_one_core() {
        assert!(StdCpu.cores().is_some_and(|cores| cores >= 1));
    }
}
