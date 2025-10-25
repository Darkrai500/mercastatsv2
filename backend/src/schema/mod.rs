pub mod auth;
pub mod ocr;

pub use auth::{AuthResponse, LoginRequest, RegisterRequest, UserInfo};
pub use ocr::TicketProcessPayload;
