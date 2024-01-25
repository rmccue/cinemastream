use anyhow::Result;
use aws_config::meta::region::RegionProviderChain;
use aws_config::BehaviorVersion;
use aws_sdk_cloudwatchlogs::Client;
use aws_sdk_cloudwatchlogs::types::InputLogEvent;
use std::io;
use std::time::Duration;
use std::time::SystemTime;
use tokio::sync::mpsc::{channel, Sender, Receiver};
use tokio::sync::mpsc::error::TrySendError;
use tokio::time::timeout;

static BUF_SIZE: usize = 2048;
static MAX_ENTRIES: usize = 2048;

pub enum LogEvent {
    Message(InputLogEvent),
    Flush,
}

/// Logger for arbitrary line-oriented data. Expected to be wrapped in a
/// LineWriter.
pub struct Logger {
    sender: Sender<LogEvent>,
}
impl Logger {
    /// Created by AsyncLogger.
    fn new(sender: Sender<LogEvent>) -> Self {
        Self {
            sender,
        }
    }

    /// Push a message to the logger.
    ///
    /// Expected to be a full line, including the newline.
    pub fn push(&self, message: String) -> Result<(), TrySendError<LogEvent>> {
        let event = InputLogEvent::builder()
            .message(message)
            .timestamp(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as i64)
            .build().unwrap();

        // eprintln!("Pushing message: {:?}", event);
        self.sender.try_send(LogEvent::Message(event))
    }

    /// Flush data to the logger.
    pub fn flush(&self) -> Result<(), TrySendError<LogEvent>> {
        self.sender.try_send(LogEvent::Flush)
    }
}

/// Wrapper for io::Error that implements From<TrySendError<LogEvent>>.
/// (Required due to Rust's orphan rules.)
pub struct IoErrorWrapper(io::Error);
impl From<TrySendError<LogEvent>> for IoErrorWrapper {
    fn from(error: TrySendError<LogEvent>) -> Self {
        match error {
            TrySendError::Full(_) => IoErrorWrapper(io::Error::new(io::ErrorKind::OutOfMemory, "Channel is full")),
            TrySendError::Closed(_) => IoErrorWrapper(io::Error::new(io::ErrorKind::BrokenPipe, "Channel is closed")),
        }
    }
}
impl From<IoErrorWrapper> for io::Error {
    fn from(wrapper: IoErrorWrapper) -> io::Error {
        wrapper.0
    }
}

/// Implementation for io::Write.
impl io::Write for Logger {
    /// Write message to CloudWatch Logs.
    ///
    /// Expected to be a full line via io::LineWriter.
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let message = String::from_utf8_lossy(buf).to_string();
        match self.push(message) {
            Ok(_) => {
                Ok(buf.len())
            },
            Err(e) => {
                // println!("Error: {:?}", e);
                let err: IoErrorWrapper = e.into();
                return Err(err.into())
            }
        }
    }

    /// Force a flush.
    fn flush(&mut self) -> std::io::Result<()> {
        Logger::flush(self).map_err( | e | {
            let err: IoErrorWrapper = e.into();
            err.into()
        } )
    }
}

pub struct AsyncLogger {
    group: String,
    id: String,
    client: Client,
    receiver: Receiver<LogEvent>,
    interval: Duration,
    // stream: Stream,
}
impl AsyncLogger {
    pub async fn create(group: String, id: String) -> Result<Logger> {
		let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
		let config = aws_config::defaults(BehaviorVersion::latest())
			.region(region_provider)
			.load()
			.await;
		let client = Client::new(&config);

        // Ensure our log stream exists.
        match client.create_log_stream()
            .log_group_name(group.clone())
            .log_stream_name(id.clone())
            .send()
            .await {
                Ok(_) => Ok(()),
                Err(e) => match e.into_service_error().into() {
                    aws_sdk_cloudwatchlogs::Error::ResourceAlreadyExistsException(_) => {
                        // Ignore existing log streams.
                        Ok(())
                    },
                    err => Err(err),
                },
            }?;

        let (sender, receiver) = channel(BUF_SIZE);
        let mut relay = Self {
            receiver,
            group,
            id,
            client,
            interval: Duration::from_secs(1)
        };

        tokio::spawn(async move {
            let _ = relay.run().await;
        });
        Ok(Logger::new(sender))
    }

    async fn run(&mut self) -> Result<()> {
        let mut buffer = Vec::with_capacity(MAX_ENTRIES);

        loop {
            match timeout(self.interval, self.receiver.recv()).await {
                Ok(Some(event)) => {
                    match event {
                        LogEvent::Message(message) => {
                            buffer.push(message);

                            if buffer.len() >= MAX_ENTRIES {
                                self.send(
                                    core::mem::take(&mut buffer)
                                ).await?;
                            }
                        },
                        LogEvent::Flush => {
                            if buffer.len() > 0 {
                                self.send(
                                    core::mem::take(&mut buffer)
                                ).await?;
                            }
                        }
                    }
                },
                Ok(None) => {
                    // The channel was closed
                    break;
                },
                Err(_) => {
                    // todo!
                    // return Err(());
                }
            }
            if !buffer.is_empty() {
                self.send(
                    core::mem::take(&mut buffer)
                ).await?;
            }
        }

        Ok(())
    }

    async fn send(&self, buffer: Vec<InputLogEvent>) -> Result<()> {
        self.client.put_log_events()
            .log_group_name(self.group.clone())
            .log_stream_name(self.id.clone())
            .set_log_events(Some(buffer))
            .send().await?;

        Ok(())
    }
}
