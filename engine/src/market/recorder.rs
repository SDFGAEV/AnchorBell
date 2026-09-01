use std::io::{self, Write};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordedMarketMessage {
    pub received_at_ms: i64,
    pub payload: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RecorderError {
    Io,
    OutOfOrder { previous_ms: i64, current_ms: i64 },
}

pub struct JsonlRecorder<W> {
    writer: W,
    last_received_at_ms: Option<i64>,
}

impl<W: Write> JsonlRecorder<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            last_received_at_ms: None,
        }
    }

    pub fn append(&mut self, record: RecordedMarketMessage) -> Result<(), RecorderError> {
        if let Some(previous_ms) = self.last_received_at_ms {
            if record.received_at_ms < previous_ms {
                return Err(RecorderError::OutOfOrder {
                    previous_ms,
                    current_ms: record.received_at_ms,
                });
            }
        }
        serde_json::to_writer(&mut self.writer, &record).map_err(|_| RecorderError::Io)?;
        self.writer
            .write_all(b"\n")
            .map_err(|_| RecorderError::Io)?;
        self.last_received_at_ms = Some(record.received_at_ms);
        Ok(())
    }

    pub fn finish(mut self) -> Result<W, RecorderError> {
        self.writer.flush().map_err(|_| RecorderError::Io)?;
        Ok(self.writer)
    }
}

impl From<io::Error> for RecorderError {
    fn from(_: io::Error) -> Self {
        Self::Io
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_one_json_record_per_line() {
        let mut output = Vec::new();
        let mut recorder = JsonlRecorder::new(&mut output);
        recorder
            .append(RecordedMarketMessage {
                received_at_ms: 10,
                payload: "{}".to_string(),
            })
            .unwrap();
        recorder.finish().unwrap();
        assert!(String::from_utf8(output).unwrap().ends_with("\n"));
    }

    #[test]
    fn rejects_out_of_order_receipts() {
        let mut recorder = JsonlRecorder::new(Vec::new());
        recorder
            .append(RecordedMarketMessage {
                received_at_ms: 10,
                payload: "{}".to_string(),
            })
            .unwrap();
        assert!(matches!(
            recorder.append(RecordedMarketMessage {
                received_at_ms: 9,
                payload: "{}".to_string(),
            }),
            Err(RecorderError::OutOfOrder { .. })
        ));
    }
}
