/// Request-level failures. Each transport maps these onto its own status codes.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Unavailable(String),
    /// Broken invariant in stored data; never caused by the request itself.
    #[error("{0}")]
    Internal(String),
}

impl Error {
    pub fn status(&self) -> u16 {
        match self {
            Error::NotFound(_) => 404,
            Error::BadRequest(_) => 400,
            Error::Conflict(_) => 409,
            Error::Unavailable(_) => 503,
            Error::Internal(_) => 500,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn bad(msg: impl Into<String>) -> Error {
    Error::BadRequest(msg.into())
}
