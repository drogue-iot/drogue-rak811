use crate::protocol::Response;

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum DriverError {
    WriteError,
    ReadError,
    NotInitialized,
    UnexpectedResponse(Response),
}
