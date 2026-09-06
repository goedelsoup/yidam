// The gate is written on the `mod` declaration, not in the file it admits — which is why
// `absences` walks the parents rather than asking a file whether it is gated.
mod parse;
#[cfg(feature = "index")]
mod embedding;
