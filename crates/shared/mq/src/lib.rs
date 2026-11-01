//! RabbitMQ integration: a topic exchange (`cbtc.events` by default)
//! that every service publishes `DomainEvent`s to and can bind queues
//! against using the routing keys derived in `cbtc-events`.
//!
//! Failure simulation (duplicate delivery, delayed delivery, dropped
//! publish) is applied here at the publish boundary — this is the other
//! half of the PTDF interception seam alongside `EventEnvelope`.

use cbtc_config::{FaultInjectionConfig, RabbitMqConfig};
use cbtc_errors::AppError;
use cbtc_events::{DomainEvent, EventEnvelope};
use cbtc_ptdf::PtdfRecorder;
use futures::StreamExt;
use lapin::{
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicPublishOptions,
        ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
    },
    types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};
use rand::Rng;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

pub struct MqClient {
    channel: Channel,
    exchange: String,
    dlx_name: String,
    fault_injection: FaultInjectionConfig,
    /// Set via `with_recording` when this client instance should mirror
    /// every published event into PTDF. `None` (the default) means
    /// publishing has zero PTDF overhead — recording is opt-in per
    /// process, not a mandatory cost every service pays.
    recording: Option<(Arc<PtdfRecorder>, uuid::Uuid)>,
}

impl MqClient {
    pub async fn connect(
        cfg: &RabbitMqConfig,
        fault_injection: FaultInjectionConfig,
    ) -> Result<Self, AppError> {
        let conn = Connection::connect(&cfg.url, ConnectionProperties::default())
            .await
            .map_err(|e| AppError::MessageBroker(format!("connect failed: {e}")))?;

        let channel = conn
            .create_channel()
            .await
            .map_err(|e| AppError::MessageBroker(format!("channel open failed: {e}")))?;

        channel
            .exchange_declare(
                &cfg.exchange,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| AppError::MessageBroker(format!("exchange declare failed: {e}")))?;

        // Dead-letter exchange + queue: every consumer queue this client
        // declares (see `consume`) is set up with
        // `x-dead-letter-exchange` pointing here, so a message that is
        // permanently rejected (see the nack/requeue logic in `consume`)
        // lands somewhere an operator can inspect, rather than
        // vanishing — Phase 9's requirement that a permanently-invalid
        // message be "reject/dead-letter appropriately", not silently
        // dropped.
        let dlx_name = format!("{}.dlx", cfg.exchange);
        channel
            .exchange_declare(
                &dlx_name,
                ExchangeKind::Fanout,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                AppError::MessageBroker(format!("dead-letter exchange declare failed: {e}"))
            })?;

        channel
            .queue_declare(
                "cbtc.dead-letter",
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                AppError::MessageBroker(format!("dead-letter queue declare failed: {e}"))
            })?;

        channel
            .queue_bind(
                "cbtc.dead-letter",
                &dlx_name,
                "#",
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| AppError::MessageBroker(format!("dead-letter queue bind failed: {e}")))?;

        info!(exchange = %cfg.exchange, dlx = %dlx_name, "connected to RabbitMQ");

        Ok(Self {
            channel,
            dlx_name,
            exchange: cfg.exchange.clone(),
            fault_injection,
            recording: None,
        })
    }

    /// Enables PTDF mirroring for every subsequent `publish()` call on
    /// this client: each successfully-constructed event is also
    /// persisted to `recorder` under `run_id`, in addition to being
    /// published to RabbitMQ as normal. Intended to be called once,
    /// right after `connect`, when a service is started with PTDF
    /// recording active for the current run (see
    /// `crates/shared/ptdf/src/recorder.rs` for the recording model).
    pub fn with_recording(
        mut self,
        recorder: std::sync::Arc<PtdfRecorder>,
        run_id: uuid::Uuid,
    ) -> Self {
        self.recording = Some((recorder, run_id));
        self
    }

    /// Publish an event, applying configured fault injection (delay,
    /// drop, duplicate) before the real publish call. When fault
    /// injection is disabled this is a single publish with no overhead.
    pub async fn publish(&self, envelope: &EventEnvelope<DomainEvent>) -> Result<(), AppError> {
        if let Some((recorder, run_id)) = &self.recording {
            if let Err(e) = recorder.record(*run_id, envelope).await {
                // A PTDF recording failure must never block or fail the
                // actual platform operation the event represents —
                // recording is diagnostic tooling layered on top of the
                // real system, not a dependency of it. Log loudly and
                // continue to the real publish below.
                tracing::warn!(error = %e, event_id = %envelope.event_id, "PTDF recording failed for published event");
            }
        }

        let routing_key = envelope.payload.routing_key();
        let body = serde_json::to_vec(envelope)?;

        if self.fault_injection.enabled {
            if self.fault_injection.message_delay_ms_max > 0 {
                let delay =
                    rand::thread_rng().gen_range(0..=self.fault_injection.message_delay_ms_max);
                if delay > 0 {
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }

            let drop_roll: f64 = rand::thread_rng().gen();
            if drop_roll < self.fault_injection.packet_loss_probability {
                warn!(routing_key, "fault injection: simulated dropped MQ publish");
                return Ok(());
            }

            self.do_publish(&routing_key, &body).await?;

            let dup_roll: f64 = rand::thread_rng().gen();
            if dup_roll < self.fault_injection.duplicate_message_probability {
                warn!(
                    routing_key,
                    "fault injection: simulated duplicate MQ publish"
                );
                self.do_publish(&routing_key, &body).await?;
            }

            return Ok(());
        }

        self.do_publish(&routing_key, &body).await
    }

    async fn do_publish(&self, routing_key: &str, body: &[u8]) -> Result<(), AppError> {
        self.channel
            .basic_publish(
                &self.exchange,
                routing_key,
                BasicPublishOptions::default(),
                body,
                BasicProperties::default().with_delivery_mode(2), // persistent
            )
            .await
            .map_err(|e| AppError::MessageBroker(format!("publish failed: {e}")))?
            .await
            .map_err(|e| AppError::MessageBroker(format!("publisher confirm failed: {e}")))?;
        Ok(())
    }

    /// Declare a durable queue bound to one or more routing-key patterns
    /// (e.g. `cbtc.train.*`) and return a stream handler future the
    /// caller spawns. `handler` receives the deserialized envelope.
    ///
    /// Ack/nack policy (Phase 9): a handler that returns `Ok(())` is
    /// ACKed. A handler that returns `Err` on a message's FIRST delivery
    /// is NACKed with `requeue: true` — one retry, to absorb a
    /// transient failure (a momentarily-unreachable peer service, a
    /// brief DB blip). If the SAME message comes back already marked
    /// `redelivered` and fails again, it is NACKed with `requeue: false`,
    /// which RabbitMQ routes to the dead-letter exchange declared in
    /// `connect` — a permanently-failing message is never retried
    /// forever and never silently dropped. A message that fails to
    /// deserialize at all is treated the same as a permanent failure
    /// (straight to dead-letter), since retrying a malformed payload
    /// can never succeed.
    pub async fn consume<F, Fut>(
        &self,
        queue_name: &str,
        binding_patterns: &[&str],
        handler: F,
    ) -> Result<(), AppError>
    where
        F: Fn(EventEnvelope<DomainEvent>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), AppError>> + Send,
    {
        let mut queue_args = FieldTable::default();
        queue_args.insert(
            "x-dead-letter-exchange".into(),
            lapin::types::AMQPValue::LongString(self.dlx_name.clone().into()),
        );

        self.channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                queue_args,
            )
            .await
            .map_err(|e| AppError::MessageBroker(format!("queue declare failed: {e}")))?;

        for pattern in binding_patterns {
            self.channel
                .queue_bind(
                    queue_name,
                    &self.exchange,
                    pattern,
                    QueueBindOptions::default(),
                    FieldTable::default(),
                )
                .await
                .map_err(|e| AppError::MessageBroker(format!("queue bind failed: {e}")))?;
        }

        let mut consumer = self
            .channel
            .basic_consume(
                queue_name,
                &format!("{queue_name}-consumer"),
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| AppError::MessageBroker(format!("consume setup failed: {e}")))?;

        while let Some(delivery) = consumer.next().await {
            let delivery = match delivery {
                Ok(d) => d,
                Err(e) => {
                    warn!(error = %e, "MQ delivery error");
                    continue;
                }
            };

            let redelivered = delivery.redelivered;

            match serde_json::from_slice::<EventEnvelope<DomainEvent>>(&delivery.data) {
                Ok(envelope) => match handler(envelope).await {
                    Ok(()) => {
                        if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                            warn!(error = %e, "failed to ack MQ message");
                        }
                    }
                    Err(e) if !redelivered => {
                        warn!(error = %e, "handler failed on first delivery; nacking with requeue for one retry");
                        if let Err(e) = delivery
                            .nack(BasicNackOptions {
                                requeue: true,
                                ..Default::default()
                            })
                            .await
                        {
                            warn!(error = %e, "failed to nack MQ message for retry");
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "handler failed again on redelivery; routing to dead-letter");
                        if let Err(e) = delivery
                            .nack(BasicNackOptions {
                                requeue: false,
                                ..Default::default()
                            })
                            .await
                        {
                            warn!(error = %e, "failed to nack MQ message to dead-letter");
                        }
                    }
                },
                Err(e) => {
                    tracing::error!(error = %e, "failed to deserialize MQ message; routing to dead-letter rather than dropping or retrying");
                    if let Err(e) = delivery
                        .nack(BasicNackOptions {
                            requeue: false,
                            ..Default::default()
                        })
                        .await
                    {
                        warn!(error = %e, "failed to nack malformed MQ message to dead-letter");
                    }
                }
            }
        }

        Ok(())
    }
}
