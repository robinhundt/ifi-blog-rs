use std::fmt::{Display, Write};

pub(crate) fn display_iterable<T>(it: T) -> String
where
    T: IntoIterator,
    T::Item : Display {
    let mut buf = String::new();
    for el in it {
        writeln!(&mut buf, "{}", el).unwrap();
    }
    buf
}