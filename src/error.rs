use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Configuration,
    Input,
    Network,
    Response,
    Validation,
    Delegation,
    Workspace,
    Preview,
    Report,
}

#[derive(Debug)]
pub struct AppError {
    kind: ErrorKind,
    code: Option<&'static str>,
    message: String,
}

impl AppError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            code: None,
            message: message.into(),
        }
    }

    pub fn coded(kind: ErrorKind, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            code: Some(code),
            message: message.into(),
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn code(&self) -> Option<&'static str> {
        self.code
    }
}

impl Display for AppError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        if let Some(code) = self.code {
            write!(formatter, "{code}: {}", self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl Error for AppError {}
