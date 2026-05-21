// Default font bytes embedded into the crate. Callers can supply their own
// via `Fonts { sans_bold, script }` if they want a different type pairing.

pub(crate) static DEJAVU_SANS_BOLD: &[u8] = include_bytes!("../assets/DejaVuSans-Bold.ttf");
pub(crate) static ITALIANNO_REGULAR: &[u8] = include_bytes!("../assets/Italianno-Regular.ttf");
