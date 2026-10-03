use crate::ports::Cpu;

/// The `Cpu` of a `Context` that was not given one: it knows nothing.
pub struct NoCpu;

impl Cpu for NoCpu {
    fn cores(&self) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_nothing() {
        assert_eq!(NoCpu.cores(), None);
    }
}
