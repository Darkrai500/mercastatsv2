pub mod auth;
pub mod ocr;
pub mod ticket_ingestion;

pub use auth::{generate_jwt, hash_password, verify_password};
pub use ocr::{
    process_ticket as process_ticket_ocr, OcrError,
    ProcessTicketRequest as OcrProcessTicketRequest,
    ProcessTicketResponse as OcrProcessTicketResponse, TicketProduct,
};
pub use ticket_ingestion::{ingest_ticket, TicketIngestionResponse};
