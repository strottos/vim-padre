pub mod debugger;
pub mod server;
pub mod util;

use server::PadreError;

pub type Result<T> = std::result::Result<T, PadreError>;
