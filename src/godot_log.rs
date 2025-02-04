use std::cell::Cell;
use std::io::{IoSlice, Result as IoResult, Write};
use std::str::from_utf8;

use anyhow::Result as AnyResult;
use godot::global::{print, print_rich, push_error, push_warning};
use godot::prelude::*;
use log::Record;
use log4rs::append::Append;
use log4rs::config::{Deserialize as LogDeserialize, Deserializers};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::encode::writer::simple::SimpleWriter;
use log4rs::encode::{Color, Encode, EncoderConfig, Style, Write as LogWrite};
use memchr::memchr2_iter;
use scopeguard::guard;
use serde::Deserialize;

#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, Default, Deserialize)]
pub enum GodotAppenderEmit {
    Error,
    Warning,
    #[default]
    Info,
    Rich,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize)]
pub struct GodotAppenderConfig {
    pub encoder: Option<EncoderConfig>,
    #[serde(rename = "type", default)]
    pub r#type: GodotAppenderEmit,
}

#[derive(Debug)]
pub struct GodotAppender {
    encoder: Box<dyn Encode>,
    ty: GodotAppenderEmit,
}

thread_local! {
    static TMP_STR: Cell<Vec<u8>> = Default::default();
}

impl Append for GodotAppender {
    fn append(&self, record: &Record<'_>) -> AnyResult<()> {
        let mut s = guard(TMP_STR.take(), |mut a| {
            a.clear();
            let b = TMP_STR.take();
            TMP_STR.set(if a.capacity() > b.capacity() { a } else { b });
        });
        s.clear();

        if self.ty == GodotAppenderEmit::Rich {
            let mut w = BBCodeWriter::new(&mut s);
            self.encoder.encode(&mut w, record)?;
            w.flush()?;
        } else {
            self.encoder.encode(&mut SimpleWriter(&mut *s), record)?;
        }

        let s = from_utf8(&s)?.to_variant();
        match self.ty {
            GodotAppenderEmit::Error => push_error(&[s]),
            GodotAppenderEmit::Warning => push_warning(&[s]),
            GodotAppenderEmit::Info => print(&[s]),
            GodotAppenderEmit::Rich => print_rich(&[s]),
        }

        Ok(())
    }

    fn flush(&self) {}
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct GodotAppenderDeserializer;

impl LogDeserialize for GodotAppenderDeserializer {
    type Trait = dyn Append;
    type Config = GodotAppenderConfig;

    // Required method
    fn deserialize(
        &self,
        config: Self::Config,
        deserializers: &Deserializers,
    ) -> AnyResult<Box<Self::Trait>> {
        Ok(Box::new(GodotAppender {
            encoder: match config.encoder {
                Some(e) => deserializers.deserialize(&e.kind, e.config)?,
                None => <Box<PatternEncoder>>::default(),
            },
            ty: config.r#type,
        }))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
enum BBCodeTag {
    #[default]
    None,
    Bold,
    BgColor(Color),
    Color(Color),
}

impl BBCodeTag {
    fn open<T: Write>(self, w: &mut T) -> IoResult<()> {
        fn cmap(c: Color) -> &'static str {
            match c {
                Color::Black => "black",
                Color::Red => "red",
                Color::Green => "green",
                Color::Blue => "blue",
                Color::Yellow => "yellow",
                Color::Cyan => "cyan",
                Color::Magenta => "magenta",
                Color::White => "white",
            }
        }

        match self {
            Self::None => Ok(()),
            Self::Bold => write!(w, "[b]"),
            Self::BgColor(c) => write!(w, "[bgcolor={}]", cmap(c)),
            Self::Color(c) => write!(w, "[color={}]", cmap(c)),
        }
    }

    fn close<T: Write>(self, w: &mut T) -> IoResult<()> {
        w.write_all(match self {
            Self::None => return Ok(()),
            Self::Bold => b"[/b]",
            Self::BgColor(_) => b"[/bgcolor]",
            Self::Color(_) => b"[/color]",
        })
    }
}

#[derive(Debug)]
struct BBCodeWriter<'a> {
    inner: &'a mut Vec<u8>,

    tag_stack: [BBCodeTag; 3],
    tag_stack_len: usize,
}

impl Write for BBCodeWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        self.pop_tags(0)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> IoResult<usize> {
        let mut l = 0;
        for b in bufs {
            self.write_all(b)?;
            l += b.len();
        }
        Ok(l)
    }

    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        let mut i = 0;
        for j in memchr2_iter(b'[', b']', buf) {
            self.inner.extend_from_slice(&buf[i..j]);
            match buf[j] {
                b'[' => self.inner.extend_from_slice(b"[lb]"),
                b']' => self.inner.extend_from_slice(b"[rb]"),
                _ => unreachable!(),
            }
            i = j + 1;
        }
        self.inner.extend_from_slice(&buf[i..]);
        Ok(())
    }
}

impl LogWrite for BBCodeWriter<'_> {
    fn set_style(&mut self, style: &Style) -> IoResult<()> {
        let bold = style.intense.unwrap_or(false);
        let bg_color = style.background;
        let color = style.text;

        // Try to close tags
        if let Some(i) = self.tag_stack[..self.tag_stack_len]
            .iter()
            .position(|t| match *t {
                BBCodeTag::Bold => !bold,
                BBCodeTag::BgColor(c) => Some(c) != bg_color,
                BBCodeTag::Color(c) => Some(c) != color,
                BBCodeTag::None => false,
            })
        {
            self.pop_tags(i)?;
        }

        // Check state
        let mut this_bold = false;
        let mut this_bg_color = None;
        let mut this_color = None;
        for t in self.tag_stack[..self.tag_stack_len].iter() {
            match *t {
                BBCodeTag::Bold => this_bold = true,
                BBCodeTag::BgColor(c) => this_bg_color = Some(c),
                BBCodeTag::Color(c) => this_color = Some(c),
                BBCodeTag::None => (),
            }
        }

        // Push tags
        if bg_color != this_bg_color {
            if let Some(c) = bg_color {
                self.push_tag(BBCodeTag::BgColor(c))?;
            }
        }
        if color != this_color {
            if let Some(c) = color {
                self.push_tag(BBCodeTag::Color(c))?;
            }
        }
        if bold && !this_bold {
            self.push_tag(BBCodeTag::Bold)?;
        }

        Ok(())
    }
}

impl<'a> BBCodeWriter<'a> {
    fn new(v: &'a mut Vec<u8>) -> Self {
        Self {
            inner: v,

            tag_stack: Default::default(),
            tag_stack_len: 0,
        }
    }

    fn pop_tags(&mut self, i: usize) -> IoResult<()> {
        for t in self.tag_stack[i..self.tag_stack_len].iter().rev() {
            t.close(self.inner.by_ref())?;
        }
        self.tag_stack_len = i;
        Ok(())
    }

    fn push_tag(&mut self, t: BBCodeTag) -> IoResult<()> {
        let v = &mut self.tag_stack[self.tag_stack_len];
        *v = t;
        self.tag_stack_len += 1;
        v.open(self.inner.by_ref())
    }
}
