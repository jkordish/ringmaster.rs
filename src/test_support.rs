use std::fmt::Display;

pub fn ok<T, E>(result: std::result::Result<T, E>, context: impl Display) -> T
where
    E: Display,
{
    match result {
        Ok(value) => value,
        Err(error) => unreachable!("{context}: {error}"),
    }
}

pub fn some<T>(option: Option<T>, context: impl Display) -> T {
    option.unwrap_or_else(|| unreachable!("{context}"))
}
