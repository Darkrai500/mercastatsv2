pub mod auth;
pub mod ocr;
pub mod stats;

pub use auth::{AuthResponse, LoginRequest, RegisterRequest, UserInfo};
pub use ocr::{TicketConfirmPayload, TicketProcessPayload};
pub use stats::{DashboardStatsResponse, MonthlyEvolutionResponse};
