use std::cmp::Ordering;
use std::collections::BTreeMap;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use crate::DEPTH;

#[derive(Debug, PartialEq)]
pub(crate) struct InTick {
    pub(crate) exchange: Exchange,
    pub(crate) bids: Vec<Level>,
    pub(crate) asks: Vec<Level>,
}

pub(crate) trait ToTick {
    fn maybe_to_tick(&self) -> Option<InTick>;
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct OutTick {
    pub(crate) spread: Decimal,
    pub(crate) bids: Vec<Level>,
    pub(crate) asks: Vec<Level>,
}

impl OutTick {
    pub(crate) fn new() -> OutTick {
        OutTick {
            spread: Default::default(),
            bids: vec![],
            asks: vec![],
        }
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) enum Exchange {
    Bitstamp,
    Binance,
}

impl ToString for Exchange {
    fn to_string(&self) -> String {
        match self {
            Exchange::Bitstamp => "bitstamp".to_string(),
            Exchange::Binance => "binance".to_string(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Level {
    pub(crate) side: Side,
    pub(crate) price: Decimal,
    pub(crate) amount: Decimal,
    pub(crate) exchange: Exchange,
}

impl Level {
    pub(crate) fn new(side: Side, price: Decimal, amount: Decimal, exchange: Exchange) -> Level {
        Level{side, price, amount, exchange}
    }
}

impl Ord for Level {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.price.cmp(&other.price), &self.side) {
            (Ordering::Equal, Side::Bid) => self.amount.cmp(&other.amount),
            (Ordering::Equal, Side::Ask) => self.amount.cmp(&other.amount).reverse(),
            (ord, _) => ord,
        }
    }
}

impl PartialOrd for Level {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.price.partial_cmp(&other.price), &self.side) {
            (Some(Ordering::Equal), Side::Bid) => self.amount.partial_cmp(&other.amount),
            (Some(Ordering::Equal), Side::Ask) => self.amount.partial_cmp(&other.amount).map(Ordering::reverse),
            (ord, _) => ord,
        }
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) enum Side {
    Bid,
    Ask,
}

pub(crate) trait ToLevel {
    fn to_level(&self, side: Side) -> Level;
}

pub(crate) trait ToLevels {
    fn to_levels(&self, side: Side, depth: usize) -> Vec<Level>;
}

impl<T> ToLevels for Vec<T>
    where T: ToLevel + Clone
{
    fn to_levels(&self, side: Side, depth: usize) -> Vec<Level> {
        let levels = match self.len() > depth {
            true => self.split_at(depth).0.to_vec(), // only keep 10
            false => self.clone(),
        };

        levels.into_iter()
            .map(|l| l.to_level(side.clone()))
            .collect()
    }
}

trait Merge {
    fn merge(self, other: Vec<Level>) -> Vec<Level>;
    fn merge_map(self, other: LevelsMap) -> Vec<Level>;
}

impl Merge for Vec<Level> {
    fn merge(self, other: Vec<Level>) -> Vec<Level> {
        let mut levels: Vec<Level> =
            self.into_iter()
                .chain(other)
                .collect();
        levels.sort_unstable();
        levels
    }

    fn merge_map(self, other: LevelsMap) -> Vec<Level> {
        let levels: Vec<Level> = other.values().cloned().collect();
        self.merge(levels)
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Exchanges {
    bitstamp: OrderDepths,
    binance: OrderDepths,
}

impl Exchanges {
    pub(crate) fn new() -> Exchanges {
        Exchanges {
            bitstamp: OrderDepths::new(),
            binance: OrderDepths::new(),
        }
    }

    /// Extracts the bids and asks from the `InTick`, then adds into its corresponding
    /// orderbook of the exchange.
    pub(crate) fn update(&mut self, t: InTick) {
        match t.exchange {
            Exchange::Bitstamp => {
                self.bitstamp.bids = t.bids;
                self.bitstamp.asks = t.asks;
            },
            Exchange::Binance => {
                self.binance.bids = t.bids;
                self.binance.asks = t.asks;
            },

        }
    }

    /// Returns a new `OutTick` containing the merge bids and asks from both orderbooks.
    pub(crate) fn to_tick(&self) -> OutTick {
        let bids: Vec<Level> =
            self.bitstamp.bids.clone()
                .merge(self.binance.bids.clone())
                .into_iter().rev().take(DEPTH)
                .collect();

        let asks: Vec<Level> =
            self.bitstamp.asks.clone()
                .merge(self.binance.asks.clone())
                .into_iter().take(DEPTH)
                .collect();

        let spread = match (bids.first(), asks.first()) {
            (Some(b), Some(a)) => a.price - b.price,
            (_, _) => dec!(0),
        };

        OutTick { spread, bids, asks }
    }
}

#[derive(Debug, PartialEq)]
struct OrderDepths {
    bids: Vec<Level>,
    asks: Vec<Level>,
}

impl OrderDepths {
    fn new() -> Self {
        OrderDepths {
            bids: vec![],
            asks: vec![],
        }
    }
}

type LevelsMap = BTreeMap<Decimal, Level>;

#[derive(Debug, PartialEq)]
struct OrderDepthsMap {
    bids: LevelsMap,
    asks: LevelsMap,
}


trait ExtendAndKeep {
    fn extend_and_keep(
        &mut self,
        other: LevelsMap,
        index: usize,
    );
}

impl ExtendAndKeep for LevelsMap {
    fn extend_and_keep(&mut self, other: LevelsMap, i: usize) {
        self.extend(other);
        self.retain(|_k, v| !v.amount.eq(&dec!(0))); // remove where volume is 0
        if self.len() > i {
            let key = self.keys().collect::<Vec<&Decimal>>()[i].clone();
            self.split_off(&key);
        }
    }
}

