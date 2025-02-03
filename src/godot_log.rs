use std::cell::Cell;
use std::str::from_utf8;

use anyhow::Result as AnyResult;
use godot::global::{print, push_error, push_warning};
use godot::prelude::*;
use log::Record;
use log4rs::append::Append;
use log4rs::config::{Deserialize as LogDeserialize, Deserializers};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::encode::writer::simple::SimpleWriter;
use log4rs::encode::{Encode, EncoderConfig};
use scopeguard::guard;
use serde::Deserialize;

#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, Default, Deserialize)]
pub enum GodotAppenderEmit {
    Error,
    Warning,
    #[default]
    Info,
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
        let mut s = guard(SimpleWriter(TMP_STR.take()), |SimpleWriter(mut a)| {
            a.clear();
            let b = TMP_STR.take();
            TMP_STR.set(if a.capacity() > b.capacity() { a } else { b });
        });

        self.encoder.encode(&mut *s, record)?;
        let s = from_utf8(&s.0)?.to_variant();
        match self.ty {
            GodotAppenderEmit::Error => push_error(&[s]),
            GodotAppenderEmit::Warning => push_warning(&[s]),
            GodotAppenderEmit::Info => print(&[s]),
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
