use core::fmt;
use std::fmt::Write as FmtWrite;
use std::fs::OpenOptions;
use std::sync::Once;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{self, FormatEvent, FormatFields};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, fmt as tracingfmt};

#[macro_export]
macro_rules! unimplemented_log {
    ( $($arg:tt)* ) => {{
        tracing::warn!(
            "unimplemented: {}",
            format_args!($($arg)*),
            // file!(),
            // line!(),
        );
    }};
}

/// if `cond` is false, logs a warning with your message.
#[macro_export]
macro_rules! assert_warn {
    ($cond:expr, $($arg:tt)+) => {{
        if !$cond {
            tracing::warn!(
                target: module_path!(),
                "assertion warning: `{}` failed: {} at {}:{}",
                stringify!($cond),
                format_args!($($arg)+),
                file!(),
                line!(),
            );
        }
    }};
}

struct AlignedFormatter;

/// Visitor to extract the ts field value (for inclusion in the line header)
/// TODO revisit this approach
struct TsVisitor {
    ts: Option<String>,
}

impl tracing::field::Visit for TsVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        if field.name() == "ts" {
            self.ts = Some(format!("{:?}", value));
        }
    }
}

/// Visitor to format event fields into a string, skipping the `ts` field
/// (which is already shown in the line header).
struct FieldsVisitor<'a> {
    writer: &'a mut String,
}

impl<'a> tracing::field::Visit for FieldsVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        if field.name() == "ts" {
            return;
        }
        if field.name() == "message" {
            write!(self.writer, "{:?}", value).ok();
        } else {
            write!(self.writer, " {}={:?}", field.name(), value).ok();
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "ts" {
            return;
        }
        if field.name() == "message" {
            self.writer.push_str(value);
        } else {
            write!(self.writer, " {}={}", field.name(), value).ok();
        }
    }
}

impl<S, N> FormatEvent<S, N> for AlignedFormatter
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(&self, _ctx: &FmtContext<'_, S, N>, mut writer: format::Writer<'_>, event: &tracing::Event<'_>) -> fmt::Result {
        let metadata = event.metadata();

        // Extract ts field if present
        let mut visitor = TsVisitor { ts: None };
        event.record(&mut visitor);
        let ts_str = visitor.ts.unwrap_or_else(|| "             ".to_string());

        // Add ANSI color codes for different log levels
        let (color_level, color_reset) = match *metadata.level() {
            tracing::Level::ERROR => ("\x1b[31m", "\x1b[0m"),
            tracing::Level::WARN => ("\x1b[33m", "\x1b[0m"),
            tracing::Level::INFO => ("\x1b[32m", "\x1b[0m"),
            tracing::Level::DEBUG => ("\x1b[34m", "\x1b[0m"),
            tracing::Level::TRACE => ("\x1b[35m", "\x1b[0m"),
        };

        // Transform file path: "crates/tetra-entities/src/cmce/subentities/cc_bs.rs"
        // becomes "ts [entities/cmce] cc_bs.rs"
        let file_path = metadata.file().unwrap_or("unknown");
        let formatted_path = if let Some(src_idx) = file_path.find("/src/") {
            // Extract crate name and module path
            let before_src = &file_path[..src_idx];
            let after_src = &file_path[src_idx + 5..]; // Skip "/src/"

            // Extract the crate name (after "tetra-")
            let crate_name = if let Some(tetra_idx) = before_src.rfind("tetra-") {
                &before_src[tetra_idx + 6..]
            } else {
                before_src.rsplit('/').next().unwrap_or("unknown")
            };

            // Extract module path and filename
            if let Some(last_slash) = after_src.rfind('/') {
                let module_path = &after_src[..last_slash];
                let filename = &after_src[last_slash + 1..];
                let first_module = module_path.split('/').next().unwrap_or("");
                format!("{} [{}/{}] {}", ts_str, crate_name, first_module, filename)
            } else {
                format!("{} [{}] {}", ts_str, crate_name, after_src)
            }
        } else {
            file_path.to_string()
        };

        // Format: "LEVEL ts [module] file:line: message"
        let location = format!(
            "{}{:<5}{} {}:{}:",
            color_level,
            metadata.level(),
            color_reset,
            formatted_path,
            metadata.line().unwrap_or(0)
        );

        // Capture the message, skipping the ts field (already shown in line header)
        let mut message_buf = String::new();
        event.record(&mut FieldsVisitor { writer: &mut message_buf });

        // Check if the message starts with "->" or "<-" to reduce indentation
        let mut padding = 70; // Default alignment
        if message_buf.starts_with("->") || message_buf.starts_with("<-") {
            padding -= 3; // Reduce by 3 characters
        }

        write!(writer, "{:<width$} {}", location, message_buf, width = padding)?;
        writeln!(writer)
    }
}

// TODO FIXME clean up at some point when new formatter is stable
#[allow(dead_code)]
struct AlignedFormatterOld;
impl<S, N> FormatEvent<S, N> for AlignedFormatterOld
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(&self, ctx: &FmtContext<'_, S, N>, mut writer: format::Writer<'_>, event: &tracing::Event<'_>) -> fmt::Result {
        let metadata = event.metadata();

        // Add ANSI color codes for different log levels
        let (level_color, reset) = match *metadata.level() {
            tracing::Level::ERROR => ("\x1b[31m", "\x1b[0m"),
            tracing::Level::WARN => ("\x1b[33m", "\x1b[0m"),
            tracing::Level::INFO => ("\x1b[32m", "\x1b[0m"),
            tracing::Level::DEBUG => ("\x1b[34m", "\x1b[0m"),
            tracing::Level::TRACE => ("\x1b[35m", "\x1b[0m"),
        };

        // Format: "LEVEL file:line: message"
        let location = format!(
            "{}{}{} {}:{}:",
            level_color,
            metadata.level(),
            reset,
            metadata.file().unwrap_or("unknown"),
            metadata.line().unwrap_or(0)
        );

        // Capture the message to check for special prefixes
        let mut message_buf = String::new();
        let message_writer = format::Writer::new(&mut message_buf);
        ctx.field_format().format_fields(message_writer, event)?;

        // Check if the message starts with "->" or "<-" to reduce indentation
        let mut padding = 60; // Default alignment
        if message_buf.starts_with("->") || message_buf.starts_with("<-") {
            padding -= 3; // Reduce by 3 characters
        }

        write!(writer, "{:<width$} {}", location, message_buf, width = padding)?;
        writeln!(writer)
    }
}

static INIT_LOG: Once = Once::new();

/// Keep non-blocking tracing workers alive for the lifetime of the process.
/// The contained guards are intentionally opaque; callers only need to hold
/// the value so background log draining continues working.
pub struct LogGuards {
    _guards: Vec<WorkerGuard>,
}

impl LogGuards {
    fn new(guards: Vec<WorkerGuard>) -> Option<Self> {
        if guards.is_empty() { None } else { Some(Self { _guards: guards }) }
    }
}

/// Sets up logging with maximum verbosity (trace level)
/// Mainly for unit tests
pub fn setup_logging_verbose() {
    let stdout_filter = EnvFilter::new("trace")
        .add_directive("quinn=info".parse().unwrap())
        .add_directive("quinn_proto=info".parse().unwrap());

    setup_logging(stdout_filter, None);
}

/// Sets up default logging to stdout and optionally, a verbose log file
/// Returns guards that must be kept alive for logging to continue working
pub fn setup_logging_default(verbose_logfile: Option<String>) -> Option<LogGuards> {
    let stdout_filter = get_default_stdout_filter();
    let logfile_and_filter = if let Some(file) = verbose_logfile {
        Some((file, get_default_logfile_filter()))
    } else {
        None
    };
    setup_logging(stdout_filter, logfile_and_filter)
}

pub fn get_default_filter() -> EnvFilter {
    EnvFilter::new("info")
}

pub fn get_default_stdout_filter() -> EnvFilter {
    EnvFilter::new("info")
        // Quinn / QUIC debug logging
        .add_directive("quinn=info".parse().unwrap())
        .add_directive("quinn_proto=info".parse().unwrap())

        // Hide continuous logs from lower layers
        .add_directive("tetra_entities::messagerouter=warn".parse().unwrap())
        .add_directive("tetra_core::bitbuffer=warn".parse().unwrap())

        // Phy
        .add_directive("tetra_entities::phy::components=info".parse().unwrap())
        // .add_directive("tetra_entities::phy::phy_bs=info".parse().unwrap())

        // Lmac
        .add_directive("tetra_entities::lmac=info".parse().unwrap())

        // Umac
        .add_directive("tetra_entities::umac::subcomp::slotter=debug".parse().unwrap())
        .add_directive("tetra_entities::umac=debug".parse().unwrap())

        // Llc
        .add_directive("tetra_entities::llc=debug".parse().unwrap())

        // Higher layers
        .add_directive("tetra_entities::mle=debug".parse().unwrap())
        .add_directive("tetra_entities::cmce=debug".parse().unwrap())
        .add_directive("tetra_entities::sndcp=debug".parse().unwrap())
        .add_directive("tetra_entities::mm=debug".parse().unwrap())
}

fn get_default_logfile_filter() -> EnvFilter {
    EnvFilter::new("debug")
}

/// Sets up logging to stdout and optionally, a verbose log file.
/// Returns guards that must be kept alive for background log draining to continue.
fn setup_logging(stdout_filter: EnvFilter, outfile: Option<(String, EnvFilter)>) -> Option<LogGuards> {
    let (stdout_writer, stdout_guard) = tracing_appender::non_blocking(std::io::stdout());
    if let Some((outfile, outfile_filter)) = outfile {
        // Setup logging with a verbose log file
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(outfile)
            .expect("Failed to open log file");
        let (file_writer, file_guard) = tracing_appender::non_blocking(file);

        // Setup once
        INIT_LOG.call_once(|| {
            let file_layer = tracingfmt::layer()
                .event_format(AlignedFormatter)
                .with_writer(file_writer)
                .with_ansi(false);

            let stdout_layer = tracingfmt::layer().event_format(AlignedFormatter).with_writer(stdout_writer);

            tracing_subscriber::registry()
                .with(file_layer.with_filter(outfile_filter))
                .with(stdout_layer.with_filter(stdout_filter))
                .init();
        });

        LogGuards::new(vec![stdout_guard, file_guard])
    } else {
        // Setup once
        INIT_LOG.call_once(|| {
            let stdout_layer = tracingfmt::layer().event_format(AlignedFormatter).with_writer(stdout_writer);

            tracing_subscriber::registry().with(stdout_layer.with_filter(stdout_filter)).init();
        });
        LogGuards::new(vec![stdout_guard])
    }
}
