// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::LazyLock;

use tracing::info;
use tracing_core::{
    callsite::Callsite,
    event::Event,
    field::{Field, FieldSet, Value},
    identify_callsite,
    metadata::{Kind, Metadata},
    subscriber::Interest,
};

#[cxx_qt::bridge]
mod ffi {
    enum LogLevel {
        Trace,
        Debug,
        Info,
        Warn,
        Error,
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qtlogging.h");
        type QtMsgType = cxx_qt_lib::QtMsgType;
        type QMessageLogContext<'a> = cxx_qt_lib::QMessageLogContext<'a>;

        include!("cpp/logging.hpp");
        fn installMessageHandler();
        fn installDefaultMessageHandler();
    }

    extern "Rust" {
        #[cxx_name = "tracingMessageHandler"]
        unsafe fn tracing_message_handler<'a>(
            msgType: QtMsgType,
            context: &QMessageLogContext<'a>,
            message: &QString,
        );
    }
}

use cxx_qt_lib::{QMessageLogContext, QString, QtMsgType};

static FIELD_NAMES: &[&str] = &["message", "qt.function", "qt.file", "qt.line"];

struct Fields {
    message: Field,
    function: Field,
    file: Field,
    line: Field,
}

impl Fields {
    fn new(cs: &'static dyn Callsite) -> Self {
        let field_set = cs.metadata().fields();
        Self {
            message: field_set.field("message").unwrap(),
            function: field_set.field("qt.function").unwrap(),
            file: field_set.field("qt.file").unwrap(),
            line: field_set.field("qt.line").unwrap(),
        }
    }
}

macro_rules! qt_cs {
    ($level:expr, $cs:ident, $meta:ident, $ty:ident) => {
        struct $ty;
        static $cs: $ty = $ty;

        static $meta: Metadata<'static> = Metadata::new(
            "qt_logging",
            "qt",
            $level,
            ::core::option::Option::None,
            ::core::option::Option::None,
            ::core::option::Option::None,
            FieldSet::new(FIELD_NAMES, identify_callsite!(&$cs)),
            Kind::EVENT,
        );

        impl Callsite for $ty {
            fn set_interest(&self, _: Interest) {}
            fn metadata(&self) -> &'static Metadata<'static> {
                &$meta
            }
        }
    };
}

qt_cs!(tracing::Level::DEBUG, DEBUG_CS, DEBUG_META, DebugCallsite);
qt_cs!(tracing::Level::INFO, INFO_CS, INFO_META, InfoCallsite);
qt_cs!(tracing::Level::WARN, WARN_CS, WARN_META, WarnCallsite);
qt_cs!(tracing::Level::ERROR, ERROR_CS, ERROR_META, ErrorCallsite);

static DEBUG_FIELDS: LazyLock<Fields> = LazyLock::new(|| Fields::new(&DEBUG_CS));
static INFO_FIELDS: LazyLock<Fields> = LazyLock::new(|| Fields::new(&INFO_CS));
static WARN_FIELDS: LazyLock<Fields> = LazyLock::new(|| Fields::new(&WARN_CS));
static ERROR_FIELDS: LazyLock<Fields> = LazyLock::new(|| Fields::new(&ERROR_CS));

fn msgtype_to_callsite(
    msg_type: QtMsgType,
) -> (
    &'static dyn Callsite,
    &'static Fields,
    &'static Metadata<'static>,
) {
    match msg_type {
        QtMsgType::QtDebugMsg => (&DEBUG_CS, &DEBUG_FIELDS, &DEBUG_META),
        QtMsgType::QtInfoMsg => (&INFO_CS, &INFO_FIELDS, &INFO_META),
        QtMsgType::QtWarningMsg => (&WARN_CS, &WARN_FIELDS, &WARN_META),
        QtMsgType::QtCriticalMsg => (&ERROR_CS, &ERROR_FIELDS, &ERROR_META),
        QtMsgType::QtFatalMsg => (&ERROR_CS, &ERROR_FIELDS, &ERROR_META),
        _ => (&INFO_CS, &INFO_FIELDS, &INFO_META),
    }
}

fn tracing_message_handler<'a>(
    msg_type: QtMsgType,
    context: &QMessageLogContext<'a>,
    message: &QString,
) {
    tracing::dispatcher::get_default(|dispatch| {
        let (_, keys, meta) = msgtype_to_callsite(msg_type);
        /*
        let qt_module = context.function();
        let qt_file = context.file();
        */
        let qt_line = context.line();

        /*
        let rust_module = qt_module.to_str().map(|s| s.to_string()).ok();
        let rust_file = qt_file.to_str().map(|s| s.to_string()).ok();
        */
        let rust_line = qt_line as u32;

        dispatch.event(&Event::new(
            meta,
            &meta.fields().value_set(&[
                (&keys.message, Some(&message.to_string() as &dyn Value)),
                (&keys.file, None),
                (&keys.function, None),
                (&keys.line, Some(&rust_line as &dyn Value)),
            ]),
        ));
    });
}

pub fn init_qt_logging() {
    ffi::installMessageHandler();
    info!("Initialized Qt logging");
}

pub fn clear_qt_logging() {
    ffi::installDefaultMessageHandler();
}
