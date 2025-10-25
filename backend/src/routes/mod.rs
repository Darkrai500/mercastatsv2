pub mod auth;
pub mod ocr;
pub mod tickets;

pub use auth::auth_router;
pub use ocr::ocr_router;
pub use tickets::tickets_router;
