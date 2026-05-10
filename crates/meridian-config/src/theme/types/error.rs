use std::fmt;

#[derive(Debug)]
pub enum ThemeError {
    Io(std::io::Error),
    Parse(toml::de::Error),
    NotFound(String),
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "IO error: {}", err),
            Self::Parse(err) => write!(f, "TOML parse error: {}", err),
            Self::NotFound(name) => write!(f, "theme not found: {}", name),
        }
    }
}

impl std::error::Error for ThemeError {}

impl From<std::io::Error> for ThemeError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<toml::de::Error> for ThemeError {
    fn from(err: toml::de::Error) -> Self {
        Self::Parse(err)
    }
}
