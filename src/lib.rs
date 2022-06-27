mod binance;
mod bitstamp;
mod error;
mod grpc;
mod orderbook;
mod websocket;
pub mod ordermaster;

pub const DEPTH:usize = 10 ;
pub const BINANCE_WS_URL: &str = "wss://stream.binance.com:9443/ws";
pub const BITSTAMP_WS_URL: &str = "wss://ws.bitstamp.net";
