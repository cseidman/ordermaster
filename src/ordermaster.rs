use crate::error::{Error, ExchangeErr};
use crate::grpc::OrderBookService;
use crate::orderbook::{Exchanges, InTick, OutTick};
use crate::{bitstamp, binance, websocket};
use futures::channel::mpsc::UnboundedSender;
use futures::{join, StreamExt};
use log::{debug, error, info};
use std::sync::Arc;
use tokio::sync::{RwLock, watch};
use tungstenite::protocol::Message;

pub async fn run(
    symbol: &String,
    port: usize,
) -> Result<(), Error>
{
    let connector = Connector::new();
    let service = OrderBookService::new(connector.out_ticks.clone());

    tokio::spawn(async move {
        service.serve(port).await.expect("Failed to serve grpc");
    });

    connector.run(symbol).await?;

    Ok(())
}

pub(crate) type OutTickPair = (watch::Sender<OutTick>, watch::Receiver<OutTick>);

struct Connector {
    out_ticks: Arc<RwLock<OutTickPair>>,
}

impl Connector {
    fn new() -> Connector {
        let out_ticks = Arc::new(RwLock::new(watch::channel(OutTick::new())));
        Connector { out_ticks }
    }

    async fn run(
        &self,
        symbol: &String,
     ) -> Result<(), Error>
    {
        let (
            ws_bitstamp,
            ws_binance,
        ) = join!(
            bitstamp::connect(symbol),
            binance::connect(symbol),
        );
        let mut ws_bitstamp = ws_bitstamp?;
        let mut ws_binance = ws_binance?;

        //let rx_stdin = stdin::rx();
        let (tx_in_ticks, mut rx_in_ticks) = futures::channel::mpsc::unbounded();

        let mut exchanges = Exchanges::new();

        // handle websocket messages
        loop {
            tokio::select! {

                ws_msg = ws_bitstamp.next() => {
                    let tx = tx_in_ticks.clone();

                    let res = handle(ws_msg)
                        .and_then(|msg| {
                             msg.parse_and_send(bitstamp::parse, tx)
                        })
                        .map_err(ExchangeErr::Bitstamp);

                    if let Err(e) = res {
                        error!("Err: {:?}", e);
                        break
                    }
                },
                ws_msg = ws_binance.next() => {
                    let tx = tx_in_ticks.clone();

                    let res = handle(ws_msg)
                        .and_then(|msg| {
                            msg.parse_and_send(binance::parse, tx)
                        })
                        .map_err(ExchangeErr::Binance);

                    if let Err(e) = res {
                        error!("Err: {:?}", e);
                        break
                    }
                },
                in_tick = rx_in_ticks.next() => {
                    match in_tick {
                        Some(t) => {
                            debug!("{:?}", t);
                            exchanges.update(t);

                            let out_tick = exchanges.to_tick();
                            debug!("{:?}", out_tick);

                            let writer = self.out_ticks.write().await;
                            let tx = &writer.0;

                            tx.send(out_tick).expect("channel should not be closed");
                        },
                        _ => {},
                    }
                },
            };
        }

        // Gracefully close connection by Close-handshake procedure
        join!(
            websocket::close(&mut ws_bitstamp),
            websocket::close(&mut ws_binance),
        );

        Ok(())
    }
}

fn handle(
    ws_msg: Option<Result<Message, tungstenite::Error>>,
) -> Result<Message, Error>
{
    let msg = ws_msg.unwrap_or_else(|| {
        info!("no message");
        Err(tungstenite::Error::ConnectionClosed)
    })?;

    Ok(msg)
}

trait ParseAndSend {
    fn parse_and_send(
        self,
        parse: fn(Message) -> Result<Option<InTick>, Error>,
        tx: UnboundedSender<InTick>,
    ) -> Result<(), Error>;
}

impl ParseAndSend for Message {
    fn parse_and_send(
        self,
        parse: fn(Message) -> Result<Option<InTick>, Error>,
        tx: UnboundedSender<InTick>,
    ) -> Result<(), Error>
    {
        parse(self).and_then(|t| {
            t.map(|tick| {
                tokio::spawn(async move {
                    tx.unbounded_send(tick).expect("Failed to send");
                });
            });
            Ok(())
        })
    }
}