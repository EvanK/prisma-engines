use std::{borrow::Cow, fmt};

use crate::value::{Function, Text, Value};

use super::attributes::FieldAttribute;

/// A field generated value.
#[derive(Debug)]
pub struct GeneratedValue<'a>(FieldAttribute<'a>);

impl<'a> GeneratedValue<'a> {
    /// A textual generated value.
    ///
    /// ```ignore
    /// model Foo {
    ///   field String @generated("id || '_copy'")
    ///                            ^^^^^^^^^^^^^ this
    /// }
    /// ```
    pub fn text(value: impl Into<Cow<'a, str>>) -> Self {
        let mut inner = Function::new("generated");
        inner.push_param(Value::from(Text::new(value)));

        Self::new(inner)
    }

    /// Sets the generated kind argument.
    ///
    /// ```ignore
    /// model Foo {
    ///   field String @generated("foo", kind: "stored")
    ///                                         ^^^^^^ this
    /// }
    /// ```
    pub fn kind(&mut self, value: impl Into<Cow<'a, str>>) {
        self.0.push_param(("kind", Text::new(value)));
    }

    fn new(inner: Function<'a>) -> Self {
        Self(FieldAttribute::new(inner))
    }
}

impl fmt::Display for GeneratedValue<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
