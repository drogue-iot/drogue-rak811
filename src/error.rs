use crate::protocol::Response;

#[derive(Debug)]
pub enum DriverError {
    WriteError,
    ReadError,
    NotInitialized,
    OtherError,
    UnexpectedResponse,
}
