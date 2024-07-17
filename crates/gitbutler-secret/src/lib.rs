pub mod secret;
pub mod sensitive;

/// A type to clearly mark sensitive information using the type-system. As such, it should
///
/// * *not* be logged
/// * *not* be stored in plain text
/// * *not* be presented in any way unless the user explicitly confirmed it to be displayed.
pub struct Sensitive<T>(pub T);
