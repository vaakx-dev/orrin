use orrin_core::Result;
use orrin_core::secret::random_hex;

pub use orrin_core::secret::same;

pub fn secret() -> Result<String> {
    random_hex(20)
}
