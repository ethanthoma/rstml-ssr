//! Server-side HTML with escaped text, typed attributes, and fallible rendering.
//!
//! ```
//! use rstml_ssr::{html, render};
//!
//! let name = "<visitor>";
//! let page = html! { <p>"Hello, "{name}</p> };
//! assert_eq!(render(&page)?.as_str(), "<p>Hello, &lt;visitor&gt;</p>");
//! # Ok::<(), rstml_ssr::RenderError>(())
//! ```
//!
//! Escaping does not validate URLs, CSS, JavaScript, or Datastar expressions.
//! Dynamic script/style content requires explicitly audited `TrustedHtml`.

use std::fmt;

#[doc(hidden)]
pub use rstml_ssr_macros::html as __html;

#[macro_export]
macro_rules! html {
    ($($tokens:tt)*) => {
        $crate::__html! { ($crate) $($tokens)* }
    };
}

pub const OUTPUT_SIZE_BYTES_MAX: u32 = 1_048_576;
pub const ITEM_COUNT_MAX: u32 = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderError {
    OutputLimit,
    Allocation,
    InvalidAttributeName,
    ItemLimit,
    NullCharacter,
    InvalidRawText,
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OutputLimit => "HTML output limit exceeded",
            Self::Allocation => "HTML output allocation failed",
            Self::InvalidAttributeName => "invalid literal HTML attribute name",
            Self::ItemLimit => "HTML list item limit exceeded",
            Self::NullCharacter => "NUL is not representable in HTML",
            Self::InvalidRawText => "raw text contains an HTML tokenizer control sequence",
        })
    }
}

impl std::error::Error for RenderError {}

pub type RenderResult = Result<(), RenderError>;

enum EscapeContext {
    Text,
    Attribute,
}

#[doc(hidden)]
pub enum RawTextElement {
    Script,
    Style,
}

/// Low-level errors leave partial output. `Output::render` clears failed documents.
pub struct Output {
    value: String,
    limit_bytes: u32,
}

impl Output {
    /// Create an empty buffer with an explicit UTF-8 byte budget.
    pub fn new(limit_bytes: u32) -> Self {
        Self {
            value: String::new(),
            limit_bytes,
        }
    }

    /// Replace the document, retaining capacity. Returned errors clear partial output.
    pub fn render(&mut self, value: &(impl Render + ?Sized)) -> Result<&str, RenderError> {
        self.value.clear();
        if let Err(error) = value.render(self) {
            self.value.clear();
            return Err(error);
        }
        Ok(self.value.as_str())
    }

    fn append(&mut self, value: &str) -> RenderResult {
        let remaining_size_bytes = (self.limit_bytes as usize).saturating_sub(self.value.len());
        if value.len() > remaining_size_bytes {
            return Err(RenderError::OutputLimit);
        }
        let required_size_bytes = self.value.len() + value.len();
        if required_size_bytes > self.value.capacity() {
            let capacity_size_bytes_target = self
                .value
                .capacity()
                .saturating_mul(2)
                .max(128)
                .max(required_size_bytes)
                .min(self.limit_bytes as usize);
            let additional_size_bytes = capacity_size_bytes_target - self.value.len();
            #[cfg(test)]
            let additional_size_bytes = tests::append_reservation_size(additional_size_bytes);
            self.value
                .try_reserve_exact(additional_size_bytes)
                .map_err(|_| RenderError::Allocation)?;
        }
        self.value.push_str(value);
        assert!(self.value.len() <= self.limit_bytes as usize);
        Ok(())
    }

    fn escaped(&mut self, value: &str, context: EscapeContext) -> RenderResult {
        let mut start_index = 0;
        for (index, byte) in value.bytes().enumerate() {
            let replacement = match byte {
                b'\0' => return Err(RenderError::NullCharacter),
                b'\r' => "&#13;",
                b'&' => "&amp;",
                b'<' => "&lt;",
                b'>' => "&gt;",
                b'"' if matches!(context, EscapeContext::Attribute) => "&quot;",
                _ => continue,
            };
            self.append(&value[start_index..index])?;
            self.append(replacement)?;
            start_index = index + 1;
        }
        self.append(&value[start_index..])
    }

    /// Append HTML-escaped text; NUL is rejected.
    pub fn text(&mut self, value: &str) -> RenderResult {
        self.escaped(value, EscapeContext::Text)
    }

    /// Append audited markup without escaping.
    pub fn trusted(&mut self, value: TrustedHtml<'_>) -> RenderResult {
        self.append(value.0)
    }

    #[doc(hidden)]
    pub fn raw_value(&mut self, value: &TrustedHtml<'_>) -> RenderResult {
        self.append(value.0)
    }

    #[doc(hidden)]
    pub fn foreign_value(&mut self, value: &TrustedHtml<'_>) -> RenderResult {
        self.text(value.0)
    }

    #[doc(hidden)]
    pub fn raw_text(
        &mut self,
        element: RawTextElement,
        render: impl FnOnce(&mut Self) -> RenderResult,
    ) -> RenderResult {
        let start_index = self.value.len();
        render(self)?;
        let value = &self.value.as_bytes()[start_index..];
        if value.contains(&0) {
            return Err(RenderError::NullCharacter);
        }
        let forbidden: &[&[u8]] = match element {
            RawTextElement::Script => &[b"</script", b"<script", b"<!--"],
            RawTextElement::Style => &[b"</style"],
        };
        for sequence in forbidden {
            if value
                .windows(sequence.len())
                .any(|window| window.eq_ignore_ascii_case(sequence))
            {
                return Err(RenderError::InvalidRawText);
            }
        }
        Ok(())
    }

    pub fn attribute(
        &mut self,
        name: &'static str,
        value: &(impl Attribute + ?Sized),
    ) -> RenderResult {
        if name.is_empty() {
            return Err(RenderError::InvalidAttributeName);
        }
        if !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
        {
            return Err(RenderError::InvalidAttributeName);
        }
        if value.present() {
            self.append(" ")?;
            self.append(name)?;
            self.append("=\"")?;
            value.render_attribute(self)?;
            self.append("\"")?;
        }
        Ok(())
    }
}

/// Bypasses escaping. Never wrap untrusted input.
///
/// ```compile_fail
/// use rstml_ssr::{Output, TrustedHtml};
/// Output::new(128).attribute("title", &TrustedHtml("<b>"));
/// ```
pub struct TrustedHtml<'a>(pub &'a str);

/// Implementations must propagate output errors.
///
/// ```compile_fail
/// use rstml_ssr::render;
/// render(&std::net::Ipv4Addr::LOCALHOST);
/// ```
pub trait Render {
    fn render(&self, output: &mut Output) -> RenderResult;
}

#[doc(hidden)]
pub enum __Branch<L, R> {
    Left(L),
    Right(R),
}

impl<L: Render, R: Render> Render for __Branch<L, R> {
    fn render(&self, output: &mut Output) -> RenderResult {
        match self {
            Self::Left(value) => value.render(output),
            Self::Right(value) => value.render(output),
        }
    }
}

impl Render for str {
    fn render(&self, output: &mut Output) -> RenderResult {
        output.text(self)
    }
}

impl Render for String {
    fn render(&self, output: &mut Output) -> RenderResult {
        output.text(self)
    }
}

impl<T: Render + ?Sized> Render for &T {
    fn render(&self, output: &mut Output) -> RenderResult {
        (*self).render(output)
    }
}

impl Render for () {
    fn render(&self, _output: &mut Output) -> RenderResult {
        Ok(())
    }
}

impl<T: Render> Render for Option<T> {
    fn render(&self, output: &mut Output) -> RenderResult {
        match self {
            Some(value) => value.render(output),
            None => Ok(()),
        }
    }
}

impl<T: Render> Render for [T] {
    fn render(&self, output: &mut Output) -> RenderResult {
        if self.len() > ITEM_COUNT_MAX as usize {
            return Err(RenderError::ItemLimit);
        }
        for item in self {
            item.render(output)?;
        }
        Ok(())
    }
}

impl<T: Render> Render for Vec<T> {
    fn render(&self, output: &mut Output) -> RenderResult {
        self.as_slice().render(output)
    }
}

impl Render for TrustedHtml<'_> {
    fn render(&self, output: &mut Output) -> RenderResult {
        output.trusted(TrustedHtml(self.0))
    }
}

pub struct Fragment<F>(F);

pub fn fragment<F: Fn(&mut Output) -> RenderResult>(render: F) -> Fragment<F> {
    Fragment(render)
}

/// Invoke the callback on every render, rather than during construction.
pub fn lazy<F, R>(render: F) -> impl Render
where
    F: Fn() -> R,
    R: Render,
{
    fragment(move |output| render().render(output))
}

/// Render a slice in order, rejecting oversized lists before invoking callbacks.
pub fn each<'a, T, R, F>(items: &'a [T], render_item: F) -> impl Render + 'a
where
    R: Render,
    F: Fn(&'a T) -> R + 'a,
{
    fragment(move |output| {
        if items.len() > ITEM_COUNT_MAX as usize {
            return Err(RenderError::ItemLimit);
        }
        for item in items {
            render_item(item).render(output)?;
        }
        Ok(())
    })
}

impl<F: Fn(&mut Output) -> RenderResult> Render for Fragment<F> {
    fn render(&self, output: &mut Output) -> RenderResult {
        (self.0)(output)
    }
}

/// ```compile_fail
/// use rstml_ssr::Markup;
/// let markup: Markup = "<b>untrusted</b>".into();
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct Markup(String);

impl Markup {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_string(self) -> String {
        self.0
    }
}

impl Render for Markup {
    fn render(&self, output: &mut Output) -> RenderResult {
        output.append(&self.0)
    }
}

/// Produce owned markup with the default output budget. Errors return no document.
pub fn render(value: &(impl Render + ?Sized)) -> Result<Markup, RenderError> {
    render_with_limit(value, OUTPUT_SIZE_BYTES_MAX)
}

/// Produce owned markup with an explicit UTF-8 byte budget.
pub fn render_with_limit(
    value: &(impl Render + ?Sized),
    limit_bytes: u32,
) -> Result<Markup, RenderError> {
    let mut output = Output::new(limit_bytes);
    output.render(value)?;
    Ok(Markup(output.value))
}

/// `false` and `None` omit the entire attribute; empty strings remain present.
pub trait Attribute {
    fn present(&self) -> bool {
        true
    }
    fn render_attribute(&self, output: &mut Output) -> RenderResult;
}

impl Attribute for str {
    fn render_attribute(&self, output: &mut Output) -> RenderResult {
        output.escaped(self, EscapeContext::Attribute)
    }
}

impl Attribute for String {
    fn render_attribute(&self, output: &mut Output) -> RenderResult {
        output.escaped(self, EscapeContext::Attribute)
    }
}

impl<T: Attribute + ?Sized> Attribute for &T {
    fn present(&self) -> bool {
        (*self).present()
    }
    fn render_attribute(&self, output: &mut Output) -> RenderResult {
        (*self).render_attribute(output)
    }
}

impl<T: Attribute> Attribute for Option<T> {
    fn present(&self) -> bool {
        self.as_ref().is_some_and(Attribute::present)
    }
    fn render_attribute(&self, output: &mut Output) -> RenderResult {
        match self {
            Some(value) => value.render_attribute(output),
            None => Ok(()),
        }
    }
}

impl Attribute for bool {
    fn present(&self) -> bool {
        *self
    }
    fn render_attribute(&self, _output: &mut Output) -> RenderResult {
        Ok(())
    }
}

struct NumberWriter<'a> {
    output: &'a mut Output,
    result: RenderResult,
}

impl fmt::Write for NumberWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.result.is_err() {
            return Err(fmt::Error);
        }
        self.result = self.output.append(value);
        self.result.map_err(|_| fmt::Error)
    }
}

fn render_number(value: impl fmt::Display, output: &mut Output) -> RenderResult {
    use fmt::Write;
    let mut writer = NumberWriter {
        output,
        result: Ok(()),
    };
    // Only primitive numbers reach this formatter; their output is HTML-safe.
    let formatted = write!(writer, "{value}");
    assert_eq!(
        formatted.is_ok(),
        writer.result.is_ok(),
        "primitive numeric formatting must propagate output errors"
    );
    writer.result
}

macro_rules! numbers {
    ($($number:ty),* $(,)?) => { $(
        impl Render for $number {
            fn render(&self, output: &mut Output) -> RenderResult { render_number(self, output) }
        }
        impl Attribute for $number {
            fn render_attribute(&self, output: &mut Output) -> RenderResult { render_number(self, output) }
        }
    )* };
}

numbers!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64
);

#[cfg(test)]
mod tests;
