// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use core::fmt;
use cortex_m_semihosting::hio;

pub struct SemihostingWriter;

impl fmt::Write for SemihostingWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if let Ok(mut stdout) = hio::hstdout() {
            stdout.write_str(s).ok();
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        {
            use core::fmt::Write;
            let _ = write!($crate::logging::SemihostingWriter, $($arg)*);
            let _ = writeln!($crate::logging::SemihostingWriter);
        }
    };
}
