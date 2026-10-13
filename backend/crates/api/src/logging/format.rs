use grove_core::{LogFormat, time::Clock};
use serde_json::{Map, Value};
use std::{fmt, sync::Arc};
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    field::RecordFields,
    fmt::{
        FmtContext, FormattedFields,
        format::{FormatEvent, FormatFields, Writer},
    },
    registry::LookupSpan,
};

#[derive(Default)]
struct SafeVisitor(Map<String, Value>);

fn number_field(name: &str) -> bool {
    matches!(
        name,
        "status"
            | "duration_ms"
            | "latency_ms"
            | "expected_bytes"
            | "observed_bytes"
            | "size"
            | "count"
            | "processed"
            | "failed"
            | "skipped"
            | "part"
            | "rows"
            | "retention_days"
            | "tick_secs"
            | "grace_secs"
            | "max_connections"
    )
}

fn string_field(name: &str) -> bool {
    matches!(
        name,
        "event"
            | "operation"
            | "outcome"
            | "stage"
            | "error_code"
            | "surface"
            | "direction"
            | "worker"
            | "task"
            | "job"
            | "stream"
            | "reason"
            | "kind"
            | "method"
    )
}

fn id_field(name: &str) -> bool {
    matches!(
        name,
        "request_id" | "operation_id" | "run_id" | "file_id" | "file" | "lease" | "displaced"
    )
}

impl SafeVisitor {
    fn string(&mut self, field: &Field, value: &str) {
        let name = field.name();
        let valid = if id_field(name) {
            uuid::Uuid::parse_str(value).is_ok()
        } else if name == "oldest_remaining_at" {
            value.len() <= 40 && chrono::DateTime::parse_from_rfc3339(value).is_ok()
        } else if name == "day" {
            value.len() == 10 && chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
        } else if matches!(name, "storage_id" | "storage") {
            value.len() <= 64
                && value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        } else if name == "route" {
            value.len() <= 160 && !value.contains(['?', '%', '\n', '\r'])
        } else {
            string_field(name)
                && value.len() <= 64
                && value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_.".contains(&c))
        };
        if valid {
            self.0.insert(name.to_owned(), value.into());
        }
    }
}

impl Visit for SafeVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.string(field, value);
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        if number_field(field.name()) {
            self.0.insert(field.name().to_owned(), value.into());
        }
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        if number_field(field.name()) {
            self.0.insert(field.name().to_owned(), value.into());
        }
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        if field.name() == "multipart" {
            self.0.insert(field.name().to_owned(), value.into());
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if id_field(field.name())
            || string_field(field.name())
            || matches!(
                field.name(),
                "storage_id" | "storage" | "route" | "oldest_remaining_at" | "day"
            )
        {
            self.string(field, &format!("{value:?}"));
        }
    }
}

pub(super) struct SafeFields;
impl<'writer> FormatFields<'writer> for SafeFields {
    fn format_fields<R: RecordFields>(
        &self,
        mut writer: Writer<'writer>,
        fields: R,
    ) -> fmt::Result {
        let mut visitor = SafeVisitor::default();
        fields.record(&mut visitor);
        write!(writer, "{}", Value::Object(visitor.0))
    }
    fn add_fields(
        &self,
        current: &mut FormattedFields<Self>,
        fields: &tracing::span::Record<'_>,
    ) -> fmt::Result {
        let mut visitor = SafeVisitor(serde_json::from_str(&current.fields).unwrap_or_default());
        fields.record(&mut visitor);
        current.fields = Value::Object(visitor.0).to_string();
        Ok(())
    }
}

pub(super) struct SafeEvent {
    pub mode: LogFormat,
    pub clock: Arc<dyn Clock>,
    pub errors: Option<tracing_appender::non_blocking::ErrorCounter>,
}

impl<S> FormatEvent<S, SafeFields> for SafeEvent
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, SafeFields>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut record = Map::new();
        if let Some(scope) = ctx.event_scope() {
            for span in scope.from_root() {
                if let Some(fields) = span.extensions().get::<FormattedFields<SafeFields>>()
                    && let Ok(fields) = serde_json::from_str::<Map<String, Value>>(&fields.fields)
                {
                    record.extend(fields);
                }
            }
        }
        let mut visitor = SafeVisitor::default();
        event.record(&mut visitor);
        record.extend(visitor.0);
        record
            .entry("event")
            .or_insert_with(|| "runtime.message".into());
        let message = match record.get("event").and_then(Value::as_str) {
            Some("http.response") => "HTTP response created",
            Some("transfer.finished") => "Byte stream finished",
            Some("worker.run_finished") => "Maintenance run finished",
            Some("management.operation_finished") => "Management operation finished",
            Some("history.write_failed") => "History recording failed",
            Some("object.operation_finished") => "Object operation finished",
            _ => "Runtime event",
        };
        record.insert("message".into(), message.into());
        record.insert("schema_version".into(), 1.into());
        record.insert(
            "timestamp".into(),
            self.clock
                .now()
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                .into(),
        );
        record.insert(
            "level".into(),
            event
                .metadata()
                .level()
                .as_str()
                .to_ascii_lowercase()
                .into(),
        );
        record.insert("target".into(), event.metadata().target().into());
        if let Some(errors) = &self.errors {
            record.insert(
                "log_dropped_lines_total".into(),
                (errors.dropped_lines() as u64).into(),
            );
        }
        if matches!(self.mode, LogFormat::Pretty) {
            write!(writer, "{} ", event.metadata().level())?;
        }
        writeln!(writer, "{}", Value::Object(record))
    }
}
