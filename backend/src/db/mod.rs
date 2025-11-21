pub mod products;
pub mod purchases;
pub mod stats;
pub mod ticket_history;
pub mod tickets;
pub mod users;

pub use products::upsert_product;
pub use purchases::{get_purchase, insert_purchase};
pub use stats::{
    get_hourly_distribution, get_month_comparison, get_monthly_spending, get_spending_trend,
    get_top_products_by_quantity, get_top_products_by_spending, get_weekly_distribution,
    DailySpendPoint, MonthlySpendPoint, TimeDistributionPoint, TopProductItem,
};
pub use ticket_history::{get_user_stats, get_user_ticket_history, TicketHistoryItem, UserStats};
pub use tickets::insert_ticket_pdf;
pub use users::{create_user, find_user_by_email};
