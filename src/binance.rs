use crate::error::Error;
use crate::orderbook::{self, Exchange, InTick, ToLevel, ToLevels, ToTick};
use crate::{BINANCE_WS_URL, DEPTH, websocket};
use log::{debug};
use rust_decimal::Decimal;
use serde::Deserialize;
use tungstenite::Message;

#[derive(Debug, Deserialize, PartialEq)]
struct Event {
    #[serde(rename = "lastUpdateId")]
    last_update_id: usize,
    bids: Vec<Level>,
    asks: Vec<Level>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
struct Level {
    price: Decimal,
    amount: Decimal,
}

impl ToLevel for Level {
    fn to_level(&self, side: orderbook::Side) -> orderbook::Level {
        orderbook::Level::new(side, self.price, self.amount, Exchange::Binance)
    }
}

impl ToTick for Event {

    fn maybe_to_tick(&self) -> Option<InTick> {
        let bids = self.bids.to_levels(orderbook::Side::Bid, DEPTH);
        let asks = self.asks.to_levels(orderbook::Side::Ask, DEPTH);

        Some(InTick { exchange: Exchange::Binance, bids, asks })
    }
}

pub(crate) async fn connect(symbol: &String) -> Result<websocket::WsStream, Error> {
    let depth = DEPTH;
    let symbol = symbol.to_lowercase().replace("/", "");
    let url = format!("{}/{}@depth{}@100ms", BINANCE_WS_URL, symbol, depth);
    Ok(websocket::connect(url.as_str()).await?)
}

pub(crate) fn parse(msg: Message) -> Result<Option<InTick>, Error> {
    let e = match msg {
        Message::Text(x) => {
            let e= deserialize(x)?;
            debug!("{:?}", e);
            Some(e)
        },
        _ => {
            None
        }
    };
    Ok(e.map(|e| e.maybe_to_tick()).flatten())
}

fn deserialize(s: String) -> serde_json::Result<Event> {
    Ok(serde_json::from_str(&s)?)
}
