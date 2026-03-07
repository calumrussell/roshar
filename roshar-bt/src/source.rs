use anyhow::Result;
use bytes::{Bytes, BytesMut};
use flate2::read::GzDecoder;
use futures::TryStreamExt;
use log::{error, warn};
use object_store::ObjectMeta;
use object_store::{path::Path as ObjectPath, ObjectStore};
use std::collections::VecDeque;
use std::fmt::Write;
use std::fs::File;
use std::io::{BufRead, Cursor, Read};
use std::sync::Arc;
use url::Url;

use crate::types::{Candle, Event};

#[derive(Clone, Debug, PartialEq)]
pub enum EventSrcState {
    Active,
    Empty,
}

pub trait EventSrc: Send {
    fn pop(&mut self, buf: &mut String) -> Option<EventSrcState>;
}

#[derive(Clone, Debug)]
pub enum FeedState {
    Active,
    Empty,
}

/// Unified abstraction over data sources for backtesting.
///
/// Combines both event and candle production into a single trait, replacing the
/// separate `EventSrc` + `EventProducer` pair.
pub trait EventFeed: Send {
    fn fill(
        &mut self,
        events: &mut VecDeque<Event>,
        candles: &mut VecDeque<Candle>,
        count: usize,
    ) -> FeedState;
}

/// Wraps an `EventSrc` + `EventProducer` into an `EventFeed` (events only, no candles).
pub struct ParsedFeed<S: EventSrc, P: EventProducer> {
    src: S,
    parser: P,
    buf: String,
}

impl<S: EventSrc, P: EventProducer> ParsedFeed<S, P> {
    pub fn new(src: S, parser: P) -> Self {
        Self {
            src,
            parser,
            buf: String::with_capacity(1_024),
        }
    }
}

impl<S: EventSrc, P: EventProducer> EventFeed for ParsedFeed<S, P> {
    fn fill(
        &mut self,
        events: &mut VecDeque<Event>,
        _candles: &mut VecDeque<Candle>,
        count: usize,
    ) -> FeedState {
        for _ in 0..count {
            match self.src.pop(&mut self.buf) {
                Some(EventSrcState::Empty) | None => return FeedState::Empty,
                Some(EventSrcState::Active) => {
                    if let Err(e) = self.parser.parse_line(&self.buf, events) {
                        warn!("Failed to parse line: {}", e);
                    }
                }
            }
        }
        FeedState::Active
    }
}

/// Wraps an `EventSrc` + `EventProducer + CandleProducer` into an `EventFeed` (events + candles).
pub struct ParsedCandleFeed<S: EventSrc, P: EventProducer + CandleProducer> {
    src: S,
    parser: P,
    buf: String,
}

impl<S: EventSrc, P: EventProducer + CandleProducer> ParsedCandleFeed<S, P> {
    pub fn new(src: S, parser: P) -> Self {
        Self {
            src,
            parser,
            buf: String::with_capacity(1_024),
        }
    }
}

impl<S: EventSrc, P: EventProducer + CandleProducer> EventFeed for ParsedCandleFeed<S, P> {
    fn fill(
        &mut self,
        events: &mut VecDeque<Event>,
        candles: &mut VecDeque<Candle>,
        count: usize,
    ) -> FeedState {
        for _ in 0..count {
            match self.src.pop(&mut self.buf) {
                Some(EventSrcState::Empty) | None => return FeedState::Empty,
                Some(EventSrcState::Active) => {
                    if let Err(e) = self.parser.parse_candle(&self.buf, candles) {
                        warn!("Failed to parse candle: {}", e);
                    }
                    if let Err(e) = self.parser.parse_line(&self.buf, events) {
                        warn!("Failed to parse line: {}", e);
                    }
                }
            }
        }
        FeedState::Active
    }
}

/// Direct event feed from pre-built event and candle vectors.
pub struct VecEventFeed {
    events: VecDeque<Event>,
    candles: VecDeque<Candle>,
}

impl VecEventFeed {
    pub fn new(events: Vec<Event>) -> Self {
        Self {
            events: events.into(),
            candles: VecDeque::new(),
        }
    }

    pub fn with_candles(events: Vec<Event>, candles: Vec<Candle>) -> Self {
        Self {
            events: events.into(),
            candles: candles.into(),
        }
    }
}

impl EventFeed for VecEventFeed {
    fn fill(
        &mut self,
        events: &mut VecDeque<Event>,
        candles: &mut VecDeque<Candle>,
        count: usize,
    ) -> FeedState {
        let start_events = events.len();
        let start_candles = candles.len();
        for _ in 0..count {
            if let Some(event) = self.events.pop_front() {
                events.push_back(event);
            } else {
                break;
            }
        }
        for _ in 0..count {
            if let Some(candle) = self.candles.pop_front() {
                candles.push_back(candle);
            } else {
                break;
            }
        }
        if events.len() > start_events || candles.len() > start_candles {
            FeedState::Active
        } else {
            FeedState::Empty
        }
    }
}

pub struct EventVecSource {
    pub evs: VecDeque<String>,
}

impl EventVecSource {
    pub fn new(evs: Vec<String>) -> Self {
        Self { evs: evs.into() }
    }
}

impl EventSrc for EventVecSource {
    fn pop(&mut self, buf: &mut String) -> Option<EventSrcState> {
        match self.evs.pop_front() {
            None => Some(EventSrcState::Empty),
            Some(event) => {
                buf.clear();
                if let Err(e) = buf.write_str(&event) {
                    warn!("Failed to write event to buffer: {}", e);
                }
                Some(EventSrcState::Active)
            }
        }
    }
}

pub trait EventProducer: Send + Clone {
    fn parse_line(
        &mut self,
        line: &str,
        evs: &mut VecDeque<Event>,
    ) -> Result<(), serde_json::Error>;
}

pub trait CandleProducer {
    type ToCandle: Into<Candle>;

    fn parse_candle(
        &mut self,
        line: &str,
        candles: &mut VecDeque<Candle>,
    ) -> Result<(), serde_json::Error>;
}

pub trait EventWriter {
    fn write_to_queue(&self, evs: &mut VecDeque<Event>);
}

pub trait CandleWriter {
    fn write_to_candle_queue(&self, candles: &mut VecDeque<Candle>);
}

pub struct BufSource {
    pub read: Vec<Box<dyn BufRead + Send>>,
    pub buf: String,
    pub pos: usize,
}

impl BufSource {
    pub fn new(read: Vec<Box<dyn BufRead + Send>>) -> Self {
        Self {
            read,
            buf: String::new(),
            pos: 0,
        }
    }

    pub fn new_file<Pth: AsRef<std::path::Path>>(path: Pth) -> Self {
        let path_ref = path.as_ref();
        let file = File::open(path_ref).expect("Couldn't open file");
        let reader: Box<dyn BufRead + Send> = if path_ref.extension().is_some_and(|ext| ext == "gz")
        {
            Box::new(std::io::BufReader::new(GzDecoder::new(file)))
        } else {
            Box::new(std::io::BufReader::new(file))
        };
        Self::new(vec![reader])
    }

    pub fn new_files<Pth: AsRef<std::path::Path>>(paths: Vec<Pth>) -> Self {
        let mut readers = Vec::new();
        for path in paths {
            let path_ref = path.as_ref();
            let file = File::open(path_ref).expect("Couldn't open file");
            let reader: Box<dyn BufRead + Send> =
                if path_ref.extension().is_some_and(|ext| ext == "gz") {
                    Box::new(std::io::BufReader::new(GzDecoder::new(file)))
                } else {
                    Box::new(std::io::BufReader::new(file))
                };
            readers.push(reader);
        }
        Self::new(readers)
    }
}

impl EventSrc for BufSource {
    fn pop(&mut self, buf: &mut String) -> Option<EventSrcState> {
        loop {
            buf.clear();
            if let Ok(res) = self.read[self.pos].read_line(buf) {
                if res == 0 {
                    if (self.read.len() - 1) == self.pos {
                        return Some(EventSrcState::Empty);
                    } else {
                        self.pos += 1;
                        continue;
                    }
                }
            }
            return Some(EventSrcState::Active);
        }
    }
}

// Internal per-feed state used by MultiplexedFeed.
struct MuxFeedState {
    feed: Box<dyn EventFeed>,
    event_buf: VecDeque<Event>,
    candle_buf: VecDeque<Candle>,
    exhausted: bool,
}

impl MuxFeedState {
    fn new(feed: Box<dyn EventFeed>) -> Self {
        Self {
            feed,
            event_buf: VecDeque::new(),
            candle_buf: VecDeque::new(),
            exhausted: false,
        }
    }

    /// Ensure at least one event is buffered (or mark exhausted).
    fn ensure_buffered(&mut self) {
        while self.event_buf.is_empty() && !self.exhausted {
            match self.feed.fill(&mut self.event_buf, &mut self.candle_buf, 1) {
                FeedState::Empty => self.exhausted = true,
                FeedState::Active => {}
            }
        }
    }

    fn peek_ts(&mut self) -> Option<i64> {
        self.ensure_buffered();
        self.event_buf.front().map(|e| e.ts)
    }

    fn pop_event(&mut self) -> Option<Event> {
        self.ensure_buffered();
        self.event_buf.pop_front()
    }

    fn drain_candles(&mut self, out: &mut VecDeque<Candle>) {
        out.extend(self.candle_buf.drain(..));
    }
}

/// Merges multiple per-symbol [`EventFeed`]s in timestamp order.
///
/// Each call to [`EventFeed::fill`] pops events with the smallest timestamps
/// across all sub-feeds. Sub-feeds are exhausted lazily.
///
/// # Example
/// ```ignore
/// let feed_aave = ParsedCandleFeed::new(BufSource::new_file("aave.log"), HyperliquidCandleParser::new());
/// let feed_btc  = ParsedCandleFeed::new(BufSource::new_file("btc.log"),  HyperliquidCandleParser::new());
/// let mut mux = MultiplexedFeed::new(vec![Box::new(feed_aave), Box::new(feed_btc)]);
/// ```
pub struct MultiplexedFeed {
    feeds: Vec<MuxFeedState>,
}

impl MultiplexedFeed {
    pub fn new(feeds: Vec<Box<dyn EventFeed>>) -> Self {
        Self {
            feeds: feeds.into_iter().map(MuxFeedState::new).collect(),
        }
    }
}

impl EventFeed for MultiplexedFeed {
    fn fill(
        &mut self,
        evs: &mut VecDeque<Event>,
        candles: &mut VecDeque<Candle>,
        count: usize,
    ) -> FeedState {
        let mut produced = 0;
        for _ in 0..count {
            // Find the feed with the smallest next event timestamp.
            let mut min_ts = i64::MAX;
            let mut min_idx: Option<usize> = None;

            for (i, feed) in self.feeds.iter_mut().enumerate() {
                if let Some(ts) = feed.peek_ts() {
                    if ts < min_ts {
                        min_ts = ts;
                        min_idx = Some(i);
                    }
                }
            }

            match min_idx {
                None => break, // all feeds exhausted
                Some(idx) => {
                    if let Some(event) = self.feeds[idx].pop_event() {
                        evs.push_back(event);
                        produced += 1;
                    }
                    self.feeds[idx].drain_candles(candles);
                }
            }
        }

        if produced > 0 {
            FeedState::Active
        } else {
            FeedState::Empty
        }
    }
}

pub struct S3Source {
    object_store: Arc<dyn ObjectStore>,
    keys: VecDeque<String>,
    current_data: Option<Bytes>,
    current_pos: usize,
    runtime: tokio::runtime::Runtime,
    filter_path: Box<dyn Fn(&str) -> bool + Send>,
}

impl S3Source {
    pub fn new(
        endpoint: &str,
        bucket: &str,
        keys: Vec<String>,
        filter_path: fn(&str) -> bool,
    ) -> Result<Self, anyhow::Error> {
        let access_key = std::env::var("AWS_ACCESS_KEY_ID")
            .map_err(|_| anyhow::anyhow!("AWS_ACCESS_KEY_ID not set in environment"))?;
        let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY")
            .map_err(|_| anyhow::anyhow!("AWS_SECRET_ACCESS_KEY not set in environment"))?;
        let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let endpoint_string = endpoint.to_string();

        let url = Url::parse(&format!("s3://{}/", bucket))?;
        let (store, _) = object_store::parse_url_opts(
            &url,
            vec![
                ("allow_http", "true"),
                ("aws_access_key_id", &access_key),
                ("aws_secret_access_key", &secret_key),
                ("region", &region),
                ("endpoint", &endpoint_string),
            ],
        )?;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create runtime: {}", e))?;

        Ok(Self {
            object_store: Arc::new(store),
            keys: keys.into(),
            current_data: None,
            current_pos: 0,
            runtime,
            filter_path: Box::new(filter_path),
        })
    }

    fn load_next_object(&mut self) -> Result<(), anyhow::Error> {
        if let Some(key) = self.keys.pop_front() {
            let path = ObjectPath::from(key.as_str());
            let path_list = self.runtime.block_on(async {
                self.object_store
                    .list(Some(&path))
                    .try_collect::<Vec<ObjectMeta>>()
                    .await
            })?;

            let mut bytes = BytesMut::new();
            for object_meta in path_list {
                if self.filter_path.as_ref()(object_meta.location.as_ref()) {
                    let data = self
                        .runtime
                        .block_on(self.object_store.get(&object_meta.location))?
                        .bytes();
                    let file_bytes = self.runtime.block_on(data).unwrap();

                    if object_meta.location.to_string().ends_with(".gz") {
                        let cursor = Cursor::new(file_bytes);
                        let mut decoder = GzDecoder::new(cursor);
                        let mut decompressed = Vec::new();
                        decoder.read_to_end(&mut decompressed)?;
                        bytes.extend_from_slice(&Bytes::from(decompressed));
                    } else {
                        bytes.extend_from_slice(&file_bytes);
                    }
                }
            }

            self.current_data = Some(bytes.into());
            self.current_pos = 0;
        } else {
            self.current_data = None;
        }
        Ok(())
    }
}

impl EventSrc for S3Source {
    fn pop(&mut self, buf: &mut String) -> Option<EventSrcState> {
        if self.current_data.is_none()
            || self.current_pos >= self.current_data.as_ref().unwrap().len()
        {
            if let Err(e) = self.load_next_object() {
                error!("{:?}", e);
                return None;
            }
            if self.current_data.is_none() {
                return Some(EventSrcState::Empty);
            }
        }
        let data = self.current_data.as_ref().unwrap();
        let mut line_end = self.current_pos;
        while line_end < data.len() && data[line_end] != b'\n' {
            line_end += 1;
        }
        if line_end > self.current_pos {
            let line = &data[self.current_pos..line_end];
            buf.clear();
            if line.len() >= 20 {
                let line_utf8 = std::str::from_utf8(line).unwrap();
                if let Err(e) = buf.write_str(line_utf8) {
                    warn!("Failed to write line to buffer: {}", e);
                }
            }
            self.current_pos = line_end + 1;
        }
        Some(EventSrcState::Active)
    }
}

#[cfg(test)]
mod tests {
    use super::BufSource;
    use crate::{
        exchanges::hyperliquid::HyperliquidCandleParser,
        source::{EventProducer, EventSrc, EventSrcState},
    };
    use std::{
        collections::VecDeque,
        io::{BufReader, Cursor},
    };

    fn setup() -> BufReader<Cursor<&'static [u8]>> {
        let evs = r#"1742428800156187511 {"channel":"candle","data":{"T":1742428799999,"c":"182.58","h":"182.58","i":"1m","l":"182.58","n":1,"o":"182.58","s":"AAVE","t":1742428740000,"v":"0.21"}}
1742428812491429400 {"channel":"candle","data":{"T":1742428859999,"c":"182.61","h":"182.61","i":"1m","l":"182.61","n":1,"o":"182.61","s":"AAVE","t":1742428800000,"v":"0.38"}}
1742428819258898930 {"channel":"candle","data":{"T":1742428859999,"c":"182.65","h":"182.65","i":"1m","l":"182.61","n":2,"o":"182.61","s":"AAVE","t":1742428800000,"v":"0.91"}}
1742428822112876929 {"channel":"candle","data":{"T":1742428859999,"c":"182.64","h":"182.65","i":"1m","l":"182.61","n":4,"o":"182.61","s":"AAVE","t":1742428800000,"v":"1.39"}}
1742428826009954884 {"channel":"candle","data":{"T":1742428859999,"c":"182.64","h":"182.65","i":"1m","l":"182.61","n":5,"o":"182.61","s":"AAVE","t":1742428800000,"v":"1.92"}}
1742428829004979305 {"channel":"candle","data":{"T":1742428859999,"c":"182.64","h":"182.65","i":"1m","l":"182.61","n":6,"o":"182.61","s":"AAVE","t":1742428800000,"v":"2.07"}}"#;
        let bytes = evs.as_bytes();
        let cursor = Cursor::new(bytes);
        BufReader::new(cursor)
    }

    fn depth_setup() -> BufReader<Cursor<&'static [u8]>> {
        let evs = r#"1736985607502103843 {"channel":"l2Book","data":{"coin":"PURR","time":1736985607199,"levels":[[{"px":"0.27736","sz":"500.0","n":1},{"px":"0.27717","sz":"426.0","n":1},{"px":"0.27711","sz":"426.0","n":1},{"px":"0.27708","sz":"245.0","n":1},{"px":"0.27707","sz":"5787.0","n":1},{"px":"0.27705","sz":"426.0","n":1},{"px":"0.27699","sz":"426.0","n":1},{"px":"0.27693","sz":"426.0","n":1},{"px":"0.27646","sz":"723.0","n":1},{"px":"0.27624","sz":"250.0","n":1},{"px":"0.27597","sz":"1449.0","n":1},{"px":"0.27551","sz":"1836.0","n":1},{"px":"0.27541","sz":"250.0","n":1},{"px":"0.27537","sz":"2905.0","n":1},{"px":"0.275","sz":"10000.0","n":1},{"px":"0.27499","sz":"3708.0","n":2},{"px":"0.27421","sz":"10610.0","n":2},{"px":"0.27316","sz":"2661.0","n":1},{"px":"0.27291","sz":"8445.0","n":1},{"px":"0.27259","sz":"47692.0","n":1}],[{"px":"0.279","sz":"423.0","n":1},{"px":"0.27908","sz":"423.0","n":1},{"px":"0.27913","sz":"250.0","n":1},{"px":"0.27914","sz":"423.0","n":1},{"px":"0.2792","sz":"422.0","n":1},{"px":"0.27927","sz":"422.0","n":1},{"px":"0.27976","sz":"250.0","n":1},{"px":"0.27992","sz":"1850.0","n":2},{"px":"0.27993","sz":"1830.0","n":1},{"px":"0.27997","sz":"714.0","n":1},{"px":"0.28046","sz":"1426.0","n":1},{"px":"0.28074","sz":"250.0","n":1},{"px":"0.28108","sz":"2846.0","n":1},{"px":"0.2821","sz":"10000.0","n":1},{"px":"0.28211","sz":"7322.0","n":1},{"px":"0.28226","sz":"5314.0","n":1},{"px":"0.28257","sz":"5190.0","n":2},{"px":"0.28294","sz":"682.0","n":1},{"px":"0.28376","sz":"37.0","n":1},{"px":"0.28429","sz":"12813.0","n":1}]]}}
1736985608021693646 {"channel":"l2Book","data":{"coin":"PURR","time":1736985607765,"levels":[[{"px":"0.27899","sz":"40.0","n":1},{"px":"0.27736","sz":"500.0","n":1},{"px":"0.27717","sz":"426.0","n":1},{"px":"0.27711","sz":"426.0","n":1},{"px":"0.27708","sz":"245.0","n":1},{"px":"0.27707","sz":"5787.0","n":1},{"px":"0.27705","sz":"426.0","n":1},{"px":"0.27699","sz":"426.0","n":1},{"px":"0.27693","sz":"426.0","n":1},{"px":"0.27646","sz":"723.0","n":1},{"px":"0.27624","sz":"250.0","n":1},{"px":"0.27597","sz":"1449.0","n":1},{"px":"0.27551","sz":"1836.0","n":1},{"px":"0.27541","sz":"250.0","n":1},{"px":"0.27537","sz":"2905.0","n":1},{"px":"0.275","sz":"10000.0","n":1},{"px":"0.27499","sz":"3708.0","n":2},{"px":"0.27421","sz":"10610.0","n":2},{"px":"0.27316","sz":"2661.0","n":1},{"px":"0.27291","sz":"8445.0","n":1}],[{"px":"0.279","sz":"423.0","n":1},{"px":"0.27908","sz":"423.0","n":1},{"px":"0.27913","sz":"250.0","n":1},{"px":"0.27914","sz":"423.0","n":1},{"px":"0.2792","sz":"422.0","n":1},{"px":"0.27927","sz":"422.0","n":1},{"px":"0.27976","sz":"250.0","n":1},{"px":"0.27992","sz":"1850.0","n":2},{"px":"0.27993","sz":"1830.0","n":1},{"px":"0.27997","sz":"714.0","n":1},{"px":"0.28046","sz":"1426.0","n":1},{"px":"0.28074","sz":"250.0","n":1},{"px":"0.28108","sz":"2846.0","n":1},{"px":"0.2821","sz":"10000.0","n":1},{"px":"0.28211","sz":"7322.0","n":1},{"px":"0.28226","sz":"5314.0","n":1},{"px":"0.28257","sz":"5190.0","n":2},{"px":"0.28294","sz":"682.0","n":1},{"px":"0.28376","sz":"37.0","n":1},{"px":"0.28428","sz":"4000.0","n":1}]]}}
1736985608597701939 {"channel":"l2Book","data":{"coin":"PURR","time":1736985608366,"levels":[[{"px":"0.27899","sz":"80.0","n":2},{"px":"0.27717","sz":"426.0","n":1},{"px":"0.27711","sz":"426.0","n":1},{"px":"0.27708","sz":"245.0","n":1},{"px":"0.27707","sz":"5787.0","n":1},{"px":"0.27705","sz":"426.0","n":1},{"px":"0.27699","sz":"426.0","n":1},{"px":"0.27693","sz":"426.0","n":1},{"px":"0.27624","sz":"250.0","n":1},{"px":"0.27551","sz":"1836.0","n":1},{"px":"0.27541","sz":"250.0","n":1},{"px":"0.27499","sz":"3708.0","n":2},{"px":"0.27421","sz":"5140.0","n":1},{"px":"0.27316","sz":"2661.0","n":1},{"px":"0.27291","sz":"8445.0","n":1},{"px":"0.27259","sz":"47692.0","n":1},{"px":"0.27252","sz":"7726.0","n":2},{"px":"0.27237","sz":"3256.0","n":1},{"px":"0.27209","sz":"39.0","n":1},{"px":"0.2719","sz":"3825.0","n":1}],[{"px":"0.279","sz":"767.0","n":3},{"px":"0.27908","sz":"423.0","n":1},{"px":"0.27913","sz":"250.0","n":1},{"px":"0.27914","sz":"423.0","n":1},{"px":"0.2792","sz":"422.0","n":1},{"px":"0.27927","sz":"422.0","n":1},{"px":"0.27976","sz":"250.0","n":1},{"px":"0.27991","sz":"3256.0","n":1},{"px":"0.27992","sz":"1850.0","n":2},{"px":"0.27993","sz":"1830.0","n":1},{"px":"0.28074","sz":"250.0","n":1},{"px":"0.2821","sz":"10000.0","n":1},{"px":"0.28211","sz":"7322.0","n":1},{"px":"0.28257","sz":"5190.0","n":2},{"px":"0.28294","sz":"682.0","n":1},{"px":"0.28376","sz":"37.0","n":1},{"px":"0.28428","sz":"4000.0","n":1},{"px":"0.28429","sz":"12813.0","n":1},{"px":"0.28444","sz":"1500.0","n":1},{"px":"0.28489","sz":"40.0","n":1}]]}}
1736985608957138466 {"channel":"trades","data":[{"coin":"PURR","side":"A","px":"0.27899","sz":"40.0","time":1736985608767,"hash":"0x92d38081159338b16193041b8e778601cc009bdb07afadb910963709b4e6ac76","tid":344983419729275,"users":["0x14a855bdcffa67fdf47a112f5180c26d7088c85f","0xc897e2a9cf140a9035e2aebaa834ae98196c152a"]}]}"#;
        let bytes = evs.as_bytes();
        let cursor = Cursor::new(bytes);
        BufReader::new(cursor)
    }

    #[test]
    fn test_that_source_stops_reading() {
        let evs = setup();
        let mut src = BufSource::new(vec![Box::new(evs)]);
        let mut buf = String::with_capacity(1_024);

        let mut seen_empty = false;
        for _i in 0..10 {
            let res = src.pop(&mut buf);
            assert!(res.is_some());

            match res.unwrap() {
                EventSrcState::Empty => {
                    seen_empty = true;
                    break;
                }
                _ => (),
            }
        }

        assert!(seen_empty);
    }

    #[test]
    fn test_that_source_reads_all() {
        let evs = setup();
        let mut src = BufSource::new(vec![Box::new(evs)]);
        let mut buf = String::with_capacity(1_024);

        let mut count = 0;
        while let Some(state) = src.pop(&mut buf) {
            match state {
                EventSrcState::Active => {
                    count += 1;
                }
                EventSrcState::Empty => {
                    break;
                }
            }
        }
        assert_eq!(count, 6)
    }

    #[test]
    fn test_that_depth_source_stops_reading() {
        let evs = depth_setup();
        let mut src = BufSource::new(vec![Box::new(evs)]);
        let mut buf = String::with_capacity(1_024);

        let mut seen_empty = false;
        for _i in 0..10 {
            let res = src.pop(&mut buf);
            assert!(res.is_some());

            match res.unwrap() {
                EventSrcState::Empty => {
                    seen_empty = true;
                    break;
                }
                _ => (),
            }
        }

        assert!(seen_empty);
    }

    #[test]
    fn test_that_depth_source_reads_all() {
        let evs = depth_setup();
        let mut src = BufSource::new(vec![Box::new(evs)]);
        let mut buf = String::with_capacity(1_024);

        let mut count = 0;
        while let Some(state) = src.pop(&mut buf) {
            match state {
                EventSrcState::Active => {
                    count += 1;
                }
                EventSrcState::Empty => {
                    break;
                }
            }
        }
        assert_eq!(count, 4)
    }

    #[test]
    fn test_vec_event_feed_returns_active_when_last_events_delivered() {
        // When fill() drains the last events from its internal buffer, it should
        // return Active (not Empty) because events were produced in this call.
        // Empty should only be returned when zero events were produced.
        use super::{EventFeed, FeedState, VecEventFeed};
        use crate::types::{Candle, Event, EVENT_TRADE_BUY};

        let events = vec![
            Event::new(EVENT_TRADE_BUY, 1000, "100.0", "1.0"),
            Event::new(EVENT_TRADE_BUY, 2000, "101.0", "2.0"),
            Event::new(EVENT_TRADE_BUY, 3000, "102.0", "3.0"),
        ];

        let mut feed = VecEventFeed::new(events);
        let mut out_events = VecDeque::new();
        let mut out_candles: VecDeque<Candle> = VecDeque::new();

        // Request exactly 3 events (matches the total count).
        // All 3 are delivered, so the result should be Active.
        let state = feed.fill(&mut out_events, &mut out_candles, 3);
        assert_eq!(out_events.len(), 3, "All 3 events should be delivered");
        assert!(
            matches!(state, FeedState::Active),
            "Feed should return Active when events were delivered"
        );

        // Now the feed is truly exhausted — no events produced, returns Empty.
        let state2 = feed.fill(&mut out_events, &mut out_candles, 3);
        assert_eq!(out_events.len(), 3, "No new events should be added");
        assert!(
            matches!(state2, FeedState::Empty),
            "Feed should return Empty only when no events were produced"
        );
    }

    #[test]
    fn test_vec_event_feed_returns_active_when_events_remain() {
        // When the feed has more events than requested, it returns Active.
        use super::{EventFeed, FeedState, VecEventFeed};
        use crate::types::{Candle, Event, EVENT_TRADE_BUY};

        let events = vec![
            Event::new(EVENT_TRADE_BUY, 1000, "100.0", "1.0"),
            Event::new(EVENT_TRADE_BUY, 2000, "101.0", "2.0"),
            Event::new(EVENT_TRADE_BUY, 3000, "102.0", "3.0"),
        ];

        let mut feed = VecEventFeed::new(events);
        let mut out_events = VecDeque::new();
        let mut out_candles: VecDeque<Candle> = VecDeque::new();

        // Request only 2 of 3 events.
        let state = feed.fill(&mut out_events, &mut out_candles, 2);
        assert_eq!(out_events.len(), 2);
        assert!(
            matches!(state, FeedState::Active),
            "Feed should return Active when events remain"
        );
    }

    #[test]
    fn test_that_source_reads_from_multiple_files() {
        // Create two test files with different data
        let evs1 = r#"1742428800156187511 {"channel":"candle","data":{"T":1742428799999,"c":"182.58","h":"182.58","i":"1m","l":"182.58","n":1,"o":"182.58","s":"AAVE","t":1742428740000,"v":"0.21"}}
1742428812491429400 {"channel":"candle","data":{"T":1742428859999,"c":"182.61","h":"182.61","i":"1m","l":"182.61","n":1,"o":"182.61","s":"AAVE","t":1742428800000,"v":"0.38"}}"#;

        let evs2 = r#"1742428819258898930 {"channel":"candle","data":{"T":1742428869999,"c":"182.65","h":"182.65","i":"1m","l":"182.61","n":2,"o":"182.61","s":"AAVE","t":1742428800000,"v":"0.91"}}
1742428822112876929 {"channel":"candle","data":{"T":1742428879999,"c":"182.64","h":"182.65","i":"1m","l":"182.61","n":4,"o":"182.61","s":"AAVE","t":1742428800000,"v":"1.39"}}"#;

        let bytes1 = evs1.as_bytes();
        let bytes2 = evs2.as_bytes();
        let cursor1 = Cursor::new(bytes1);
        let cursor2 = Cursor::new(bytes2);
        let reader1 = BufReader::new(cursor1);
        let reader2 = BufReader::new(cursor2);
        let mut parser = HyperliquidCandleParser::new();
        let mut queue = VecDeque::new();

        let mut src = BufSource::new(vec![Box::new(reader1), Box::new(reader2)]);
        let mut buf = String::with_capacity(1_024);
        let mut store = VecDeque::new();

        // Read all events
        while let Some(state) = src.pop(&mut buf) {
            match state {
                EventSrcState::Empty => break,
                EventSrcState::Active => {
                    store.push_back(parser.parse_line(&buf, &mut queue));
                }
            }
            if let EventSrcState::Empty = state {
                break;
            }
        }
        // Verify we can read the events in order
        let mut events = Vec::new();
        while let Some(event) = queue.pop_front() {
            events.push(event);
        }
        // HyperliquidCandleParser only emits a candle when the next candle arrives,
        // so with 4 input lines we get 3 output events (the last one is held)
        assert_eq!(
            events.len(),
            3,
            "Should read 3 events (parser holds last candle)"
        );
    }

    #[test]
    fn test_multiplexed_feed_returns_events_in_timestamp_order() {
        use super::{EventFeed, FeedState, MultiplexedFeed, VecEventFeed};
        use crate::types::{Candle, Event, EVENT_TRADE_BUY};

        // Feed A: timestamps 1, 3, 5
        let feed_a: Box<dyn EventFeed> = Box::new(VecEventFeed::new(vec![
            Event::new(EVENT_TRADE_BUY, 1, "100.0", "1.0"),
            Event::new(EVENT_TRADE_BUY, 3, "102.0", "1.0"),
            Event::new(EVENT_TRADE_BUY, 5, "104.0", "1.0"),
        ]));

        // Feed B: timestamps 2, 4, 6
        let feed_b: Box<dyn EventFeed> = Box::new(VecEventFeed::new(vec![
            Event::new(EVENT_TRADE_BUY, 2, "200.0", "1.0"),
            Event::new(EVENT_TRADE_BUY, 4, "202.0", "1.0"),
            Event::new(EVENT_TRADE_BUY, 6, "204.0", "1.0"),
        ]));

        let mut mux = MultiplexedFeed::new(vec![feed_a, feed_b]);
        let mut out_events = VecDeque::new();
        let mut out_candles: VecDeque<Candle> = VecDeque::new();

        // Drain all events
        loop {
            match mux.fill(&mut out_events, &mut out_candles, 1) {
                FeedState::Active => {}
                FeedState::Empty => break,
            }
        }

        let timestamps: Vec<i64> = out_events.iter().map(|e| e.ts).collect();
        assert_eq!(timestamps, vec![1, 2, 3, 4, 5, 6]);
    }
}
