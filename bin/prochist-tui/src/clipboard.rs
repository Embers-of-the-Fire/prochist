use std::io::{self, Write};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

pub fn copy(text: &str) -> io::Result<()> {
    let sequence = format!("\x1b]52;c;{}\x1b\\", STANDARD.encode(text.as_bytes()));
    let mut out = io::stdout().lock();
    out.write_all(sequence.as_bytes())?;
    out.flush()
}
