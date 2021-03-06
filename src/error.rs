#[derive(Debug)]
pub enum DriverError {
    WriteError,
    ReadError,
    NotInitialized,
    OtherError,
    UnexpectedResponse,
}
