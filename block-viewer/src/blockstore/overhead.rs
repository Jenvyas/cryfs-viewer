use byte_unit::Byte;
use std::ops::Add;

/// Represents the overhead of a block store, i.e. how many overhead bytes are stored but not usable by call sits.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct Overhead {
    overhead: Byte,
}

impl Add for Overhead {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            overhead: self.overhead.add(other.overhead).unwrap(),
        }
    }
}

impl Overhead {
    pub fn new(overhead: Byte) -> Self {
        Self { overhead }
    }

    pub fn usable_block_size_from_physical_block_size(
        &self,
        physical_block_size: Byte,
    ) -> Result<Byte, InvalidBlockSizeError> {
        physical_block_size.subtract(self.overhead).ok_or_else(|| {
            InvalidBlockSizeError::new(format!(
                "Physical block size {} is smaller than overhead {}",
                physical_block_size, self.overhead
            ))
        })
    }

    pub fn physical_block_size_from_usable_block_size(&self, usable_block_size: Byte) -> Byte {
        usable_block_size.add(self.overhead).unwrap()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidBlockSizeError {
    message: String,
}

impl InvalidBlockSizeError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}
