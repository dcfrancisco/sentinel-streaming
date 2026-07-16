use std::{error::Error, fmt};
#[derive(Debug)]
pub struct SourceError(pub String);
impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for SourceError {}
