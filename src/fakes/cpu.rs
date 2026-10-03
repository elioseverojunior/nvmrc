use crate::ports::Cpu;

/// A machine with a fixed number of processors, or none known.
#[derive(Default)]
pub struct FakeCpu(Option<usize>);

impl FakeCpu {
    #[must_use]
    pub fn with_cores(cores: usize) -> Self {
        Self(Some(cores))
    }
}

impl Cpu for FakeCpu {
    fn cores(&self) -> Option<usize> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_what_it_was_given() {
        assert_eq!(FakeCpu::with_cores(4).cores(), Some(4));
        assert_eq!(FakeCpu::default().cores(), None);
    }
}
