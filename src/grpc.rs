use crate::error::Error;
use crate::orderbook::{self, OutTick};
use crate::ordermaster::OutTickPair;
use futures::Stream;
use log::info;
use rust_decimal::prelude::ToPrimitive;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{transport::Server, Request, Response, Status};

pub mod proto {
    tonic::include_proto!("orderbook");
}

pub struct OrderBookService {
    out_ticks: Arc<RwLock<OutTickPair>>,
}

impl OrderBookService {
    pub(crate) fn new(out_ticks: Arc<RwLock<OutTickPair>>) -> Self {
        OrderBookService { out_ticks }
    }

    pub(crate) async fn serve(self, port: usize) -> Result<(), Error>{
        let addr = format!("[::1]:{}", port);
        let addr = addr.parse()?;

        info!("Serving grpc at {}", addr);

        Server::builder()
            .add_service(proto::orderbook_aggregator_server::OrderbookAggregatorServer::new(self))
            .serve(addr)
            .await?;

        Ok(())
    }
}

impl From<OutTick> for proto::Summary {
    fn from(out_tick: OutTick) -> Self {
        let spread = out_tick.spread.to_f64().unwrap();
        let bids: Vec<proto::Level> = to_levels(&out_tick.bids);
        let asks: Vec<proto::Level> = to_levels(&out_tick.asks);

        proto::Summary{ spread, bids, asks }
    }
}

fn to_levels(levels: &Vec<orderbook::Level>) -> Vec<proto::Level> {
    levels.iter()
        .map(|l|
            proto::Level{
                exchange: l.exchange.to_string(),
                price: l.price.to_f64().unwrap(),
                amount: l.amount.to_f64().unwrap(),
            })
        .collect()
}

#[tonic::async_trait]
impl proto::orderbook_aggregator_server::OrderbookAggregator for OrderBookService {

    type BookSummaryStream =
        Pin<Box<dyn Stream<Item = Result<proto::Summary, Status>> + Send + 'static>>;

    async fn book_summary(
        &self,
        request: Request<proto::Empty>,
    ) -> Result<Response<Self::BookSummaryStream>, Status> {
        info!("Got a request: {:?}", request);

        let _req = request.into_inner();

        let mut rx_out_ticks = self.out_ticks.read().await.1.clone();

        let output = async_stream::try_stream! {
            // yield the current value
            let out_tick = rx_out_ticks.borrow().clone();
            yield proto::Summary::from(out_tick);

            while let Ok(_) = rx_out_ticks.changed().await {
                let out_tick = rx_out_ticks.borrow().clone();
                yield proto::Summary::from(out_tick);
            }
        };

        Ok(Response::new(Box::pin(output) as Self::BookSummaryStream))
    }
}

