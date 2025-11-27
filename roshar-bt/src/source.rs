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

#[derive(Clone, Debug)]
pub enum EventSrcState {
    Active,
    Empty,
}

pub trait EventSrc: Send {
    fn pop(&mut self, buf: &mut String) -> Option<EventSrcState>;
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
}
