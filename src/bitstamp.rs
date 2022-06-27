use chrono::{DateTime, Utc};
use crate::error::Error;
use crate::orderbook::{self, Exchange, InTick, ToLevel, ToLevels, ToTick};
use crate::{BITSTAMP_WS_URL, DEPTH, websocket};
use futures::SinkExt;
use log::{debug, info};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tungstenite::protocol::Message;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "event")]
enum Event {
    #[serde(rename = "data")]
    Data{data: InData, channel: Channel},

    #[serde(rename = "bts:subscribe")]
    Subscribe{data: OutSubscription},

    #[serde(rename = "bts:unsubscribe")]
    Unsubscribe{data: OutSubscription},

    #[serde(rename = "bts:subscription_succeeded")]
    SubscriptionSucceeded{data: InSubscription, channel: Channel},

    #[serde(rename = "bts:unsubscription_succeeded")]
    UnsubscriptionSucceeded{data: InSubscription, channel: Channel},

    #[serde(rename = "bts:error")]
    Error{data: InError, channel: Channel},
}

impl ToTick for Event {
    fn maybe_to_tick(&self) -> Option<InTick> {
        match self {
            Event::Data { data, .. } => {
                let bids = data.bids.to_levels(orderbook::Side::Bid, DEPTH);
                let asks = data.asks.to_levels(orderbook::Side::Ask, DEPTH);

                Some(InTick { exchange: Exchange::Bitstamp, bids, asks })
            },
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct OutSubscription {
    channel: Channel,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct InData {
    #[serde(with = "timestamp")]
    timestamp: DateTime<Utc>,

    #[serde(with = "microtimestamp")]
    microtimestamp: DateTime<Utc>,

    bids: Vec<Level>,
    asks: Vec<Level>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct InSubscription {}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct InError {
    code: Option<String>,
    message: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct Level {
    price: Decimal,
    amount: Decimal,
}

impl ToLevel for Level {
    fn to_level(&self, side: orderbook::Side) -> orderbook::Level {
        orderbook::Level::new(side, self.price, self.amount, Exchange::Bitstamp)
    }
}

type Channel = String;

pub(crate) async fn connect(symbol: &String) -> Result<websocket::WsStream, Error> {
    let mut ws_stream = websocket::connect(BITSTAMP_WS_URL).await?;
    subscribe(&mut ws_stream, symbol).await?;
    Ok(ws_stream)
}

pub(crate) fn parse(msg: Message) -> Result<Option<InTick>, Error> {
    let e = match msg {

        Message::Text(x) => {
            debug!("{:?}", x);

            let e= deserialize(x)?;
            match e {
                Event::Data{..} => debug!("{:?}", e),
                _ => info!("{:?}", e),
            }

            Some(e)
        },
        _ => {
            None
        }
        /*
        Message::Ping(x) => { info!("Ping {:?}", x); None },
        Message::Pong(x) => { info!("Pong {:?}", x); None },
        Message::Close(x) => { info!("Close {:?}", x); None },
        Message::Frame(x) => { info!("Frame {:?}", x); None },

         */
    };
    Ok(e.map(|e| e.maybe_to_tick()).flatten())
}

async fn subscribe (
    rx: &mut websocket::WsStream,
    symbol: &String,
) -> Result<(), Error>
{
    let symbol = symbol.to_lowercase().replace("/", "");
    let channel = format!("order_book_{}", symbol);
    let msg = serialize(Event::Subscribe{ data: OutSubscription { channel } })?;
    rx.send(Message::Text(msg)).await?;
    Ok(())
}

fn deserialize(s: String) -> serde_json::Result<Event> {
    Ok(serde_json::from_str(&s)?)
}

fn serialize(e: Event) -> serde_json::Result<String> {
    Ok(serde_json::to_string(&e)?)
}

mod timestamp {
    use std::str::FromStr;
    use chrono::{DateTime, Utc, TimeZone};
    use serde::{self, Deserialize, Serializer, Deserializer};

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer,
    {
        serializer.serialize_i64(date.timestamp())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
        where D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let datetime = Utc.timestamp(i64::from_str(&s).map_err(serde::de::Error::custom)?, 0);
        Ok(datetime)
    }
}

mod microtimestamp {
    use std::str::FromStr;
    use chrono::{DateTime, Utc, TimeZone};
    use serde::{self, Deserialize, Serializer, Deserializer};

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer,
    {
        serializer.serialize_i64(date.timestamp_nanos()/1000)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
        where D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let datetime = Utc.timestamp_nanos(i64::from_str(&s).map_err(serde::de::Error::custom)?*1000);
        Ok(datetime)
    }
}




