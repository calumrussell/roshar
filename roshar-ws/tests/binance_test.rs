use bytes::BytesMut;
use roshar_ws::{Client, Frame, Reader};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, ReadBuf};
use tokio::time::timeout;

const BINANCE_WSS_URL: &str = "wss://fstream.binance.com/ws";

// Mock stream that reads from a pre-filled buffer
struct MockStream {
    data: BytesMut,
    pos: usize,
}

impl MockStream {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data: BytesMut::from(data.as_slice()),
            pos: 0,
        }
    }
}

impl AsyncRead for MockStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let remaining = self.data.len() - self.pos;
        if remaining == 0 {
            return Poll::Ready(Ok(()));
        }

        let to_read = remaining.min(buf.remaining());
        if to_read > 0 {
            buf.put_slice(&self.data[self.pos..self.pos + to_read]);
            self.pos += to_read;
        }

        Poll::Ready(Ok(()))
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore] // Ignore by default since it requires network access
async fn test_binance_websocket() {
    // Connect to Binance WebSocket
    let mut client = Client::connect_tls(
        "fstream.binance.com:443",
        "fstream.binance.com",
        "/ws",
        1024 * 1024, // 1MB max frame size
        None,
    )
    .await
    .expect("Failed to connect to Binance WebSocket");

    println!("Connected to Binance WebSocket");

    // Subscribe to BTCUSDT ticker stream
    let subscribe_msg = r#"{"method":"SUBSCRIBE","params":["btcusdt@depth","ethusdt@depth","solusdt@depth"],"id":1}"#;
    let subscribe_frame = Frame::text(subscribe_msg);

    // Send subscription frame
    client
        .send_frame(&subscribe_frame)
        .await
        .expect("Failed to send subscription frame");

    println!("Sent subscription message");

    // Split and receive frames with timing
    let (mut reader, _writer) = client.split();

    let mut frame_count = 0;
    let target_frames = 1000;
    let receive_timeout = Duration::from_secs(60); // Timeout per frame

    let mut timings = Vec::new();
    let mut processing_times = Vec::new();
    let mut wait_times = Vec::new();
    let start_time = Instant::now();

    println!("Reading {} frames with urithiru...", target_frames);

    while frame_count < target_frames {
        let frame_start = Instant::now();
        match timeout(receive_timeout, reader.read_frame()).await {
            Ok(Ok(frame)) => {
                let frame_duration = frame_start.elapsed();
                timings.push(frame_duration);

                // Categorize: if < 1ms, likely processing time; if > 10ms, likely network wait
                if frame_duration < Duration::from_millis(1) {
                    processing_times.push(frame_duration);
                } else if frame_duration > Duration::from_millis(10) {
                    wait_times.push(frame_duration);
                }

                frame_count += 1;

                if frame_count % 100 == 0 || frame_count <= 10 {
                    println!(
                        "Frame #{}: opcode={:?}, fin={}, payload_len={}, time={:?}",
                        frame_count,
                        frame.opcode,
                        frame.fin,
                        frame.payload.len(),
                        frame_duration
                    );
                }
            }
            Ok(Err(e)) => {
                eprintln!("Error receiving frame: {}", e);
                break;
            }
            Err(_) => {
                eprintln!("Timeout waiting for frame #{}", frame_count + 1);
                break;
            }
        }
    }

    let total_time = start_time.elapsed();
    let avg_time = if !timings.is_empty() {
        timings.iter().sum::<Duration>() / timings.len() as u32
    } else {
        Duration::ZERO
    };
    let min_time = timings.iter().min().copied().unwrap_or(Duration::ZERO);
    let max_time = timings.iter().max().copied().unwrap_or(Duration::ZERO);

    let avg_processing = if !processing_times.is_empty() {
        processing_times.iter().sum::<Duration>() / processing_times.len() as u32
    } else {
        Duration::ZERO
    };
    let avg_wait = if !wait_times.is_empty() {
        wait_times.iter().sum::<Duration>() / wait_times.len() as u32
    } else {
        Duration::ZERO
    };

    println!("\n=== urithiru Results ===");
    println!("Total frames: {}", frame_count);
    println!("Total time: {:?}", total_time);
    println!("Average time per frame: {:?}", avg_time);
    println!("Min time per frame: {:?}", min_time);
    println!("Max time per frame: {:?}", max_time);
    println!(
        "Frames per second: {:.2}",
        frame_count as f64 / total_time.as_secs_f64()
    );
    println!(
        "Processing times (<1ms): {} frames, avg: {:?}",
        processing_times.len(),
        avg_processing
    );
    println!(
        "Wait times (>10ms): {} frames, avg: {:?}",
        wait_times.len(),
        avg_wait
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore] // Ignore by default since it requires network access
async fn test_binance_websocket_tungstenite() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    // Connect to Binance WebSocket
    let (mut ws_stream, _) = connect_async("wss://fstream.binance.com/ws")
        .await
        .expect("Failed to connect to Binance WebSocket");

    println!("Connected to Binance WebSocket (tokio_tungstenite)");

    // Subscribe to BTCUSDT ticker stream
    let subscribe_msg = r#"{"method":"SUBSCRIBE","params":["btcusdt@depth","ethusdt@depth","solusdt@depth"],"id":1}"#;
    let subscribe_message = Message::Text(subscribe_msg.to_string());

    // Send subscription message
    ws_stream
        .send(subscribe_message)
        .await
        .expect("Failed to send subscription message");

    println!("Sent subscription message");

    // Split stream and receive frames with timing
    let (_write, mut read) = ws_stream.split();

    let mut frame_count = 0;
    let target_frames = 1000;
    let receive_timeout = Duration::from_secs(60); // Timeout per frame

    let mut timings = Vec::new();
    let mut processing_times = Vec::new();
    let mut wait_times = Vec::new();
    let start_time = Instant::now();

    println!("Reading {} frames with tokio_tungstenite...", target_frames);

    while frame_count < target_frames {
        let frame_start = Instant::now();
        match timeout(receive_timeout, read.next()).await {
            Ok(Some(Ok(message))) => {
                let frame_duration = frame_start.elapsed();
                timings.push(frame_duration);

                // Categorize: if < 1ms, likely processing time; if > 10ms, likely network wait
                if frame_duration < Duration::from_millis(1) {
                    processing_times.push(frame_duration);
                } else if frame_duration > Duration::from_millis(10) {
                    wait_times.push(frame_duration);
                }

                frame_count += 1;

                let payload_len = match &message {
                    Message::Text(text) => text.len(),
                    Message::Binary(data) => data.len(),
                    _ => 0,
                };

                if frame_count % 100 == 0 || frame_count <= 10 {
                    println!(
                        "Frame #{}: message={:?}, payload_len={}, time={:?}",
                        frame_count, message, payload_len, frame_duration
                    );
                }
            }
            Ok(Some(Err(e))) => {
                eprintln!("Error receiving frame: {}", e);
                break;
            }
            Ok(None) => {
                eprintln!("Stream ended");
                break;
            }
            Err(_) => {
                eprintln!("Timeout waiting for frame #{}", frame_count + 1);
                break;
            }
        }
    }

    let total_time = start_time.elapsed();
    let avg_time = if !timings.is_empty() {
        timings.iter().sum::<Duration>() / timings.len() as u32
    } else {
        Duration::ZERO
    };
    let min_time = timings.iter().min().copied().unwrap_or(Duration::ZERO);
    let max_time = timings.iter().max().copied().unwrap_or(Duration::ZERO);

    let avg_processing = if !processing_times.is_empty() {
        processing_times.iter().sum::<Duration>() / processing_times.len() as u32
    } else {
        Duration::ZERO
    };
    let avg_wait = if !wait_times.is_empty() {
        wait_times.iter().sum::<Duration>() / wait_times.len() as u32
    } else {
        Duration::ZERO
    };

    println!("\n=== tokio_tungstenite Results ===");
    println!("Total frames: {}", frame_count);
    println!("Total time: {:?}", total_time);
    println!("Average time per frame: {:?}", avg_time);
    println!("Min time per frame: {:?}", min_time);
    println!("Max time per frame: {:?}", max_time);
    println!(
        "Frames per second: {:.2}",
        frame_count as f64 / total_time.as_secs_f64()
    );
    println!(
        "Processing times (<1ms): {} frames, avg: {:?}",
        processing_times.len(),
        avg_processing
    );
    println!(
        "Wait times (>10ms): {} frames, avg: {:?}",
        wait_times.len(),
        avg_wait
    );
}

#[tokio::test]
async fn test_synthetic_performance_urithiru() {
    use bytes::BufMut;

    // Generate synthetic WebSocket frames similar to Binance depth updates
    let mut frame_data = BytesMut::new();
    let target_frames = 10000;

    // Generate frames with varying sizes (similar to real Binance data)
    let payloads = vec![
        vec![0u8; 1089],  // Small frame
        vec![0u8; 10126], // Medium frame
        vec![0u8; 5345],  // Medium-small frame
        vec![0u8; 1344],  // Small frame
        vec![0u8; 6550],  // Medium frame
    ];

    println!("Generating {} synthetic frames...", target_frames);
    for i in 0..target_frames {
        let payload = &payloads[i % payloads.len()];
        // Create a server-to-client frame (unmasked)
        let frame = Frame {
            fin: true,
            rsv: [false, false, false],
            opcode: roshar_ws::Opcode::Text,
            masked: false,
            mask: None,
            payload: bytes::Bytes::from(payload.clone()),
        };

        let serialized = frame.serialize();
        frame_data.put_slice(&serialized);
    }

    println!("Created {} bytes of frame data", frame_data.len());

    // Create mock stream and reader
    let mock_stream = MockStream::new(frame_data.to_vec());
    let mut reader = Reader::from_stream_public(Box::new(mock_stream), 10 * 1024 * 1024);

    let mut timings = Vec::new();
    let start_time = Instant::now();

    println!(
        "Reading {} frames with urithiru (synthetic)...",
        target_frames
    );

    for i in 0..target_frames {
        let frame_start = Instant::now();
        match reader.read_frame().await {
            Ok(_frame) => {
                let frame_duration = frame_start.elapsed();
                timings.push(frame_duration);

                if i < 10 || i % 1000 == 0 {
                    println!("Frame #{}: time={:?}", i + 1, frame_duration);
                }
            }
            Err(e) => {
                eprintln!("Error at frame {}: {}", i + 1, e);
                break;
            }
        }
    }

    let total_time = start_time.elapsed();
    let avg_time = if !timings.is_empty() {
        timings.iter().sum::<Duration>() / timings.len() as u32
    } else {
        Duration::ZERO
    };
    let min_time = timings.iter().min().copied().unwrap_or(Duration::ZERO);
    let max_time = timings.iter().max().copied().unwrap_or(Duration::ZERO);
    let p50 = if timings.len() > 0 {
        let mut sorted = timings.clone();
        sorted.sort();
        sorted[timings.len() / 2]
    } else {
        Duration::ZERO
    };
    let p99 = if timings.len() > 0 {
        let mut sorted = timings.clone();
        sorted.sort();
        sorted[(timings.len() * 99) / 100]
    } else {
        Duration::ZERO
    };

    println!("\n=== urithiru Synthetic Results ===");
    println!("Total frames: {}", target_frames);
    println!("Total time: {:?}", total_time);
    println!("Average time per frame: {:?}", avg_time);
    println!("Min time per frame: {:?}", min_time);
    println!("Max time per frame: {:?}", max_time);
    println!("P50 time per frame: {:?}", p50);
    println!("P99 time per frame: {:?}", p99);
    println!(
        "Frames per second: {:.2}",
        target_frames as f64 / total_time.as_secs_f64()
    );
    println!(
        "Throughput: {:.2} MB/s",
        (frame_data.len() as f64 / 1024.0 / 1024.0) / total_time.as_secs_f64()
    );
}

#[tokio::test]
async fn test_synthetic_performance_tungstenite() {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    use tokio_tungstenite::tungstenite::protocol::frame::coding::{Data, OpCode};
    use tokio_tungstenite::tungstenite::protocol::frame::Frame as WsFrame;
    use tokio_tungstenite::{tungstenite::protocol::Role, WebSocketStream};

    // Generate synthetic WebSocket frames
    let mut frame_data = Vec::new();
    let target_frames = 10000;

    let payloads = vec![
        vec![0u8; 1089],
        vec![0u8; 10126],
        vec![0u8; 5345],
        vec![0u8; 1344],
        vec![0u8; 6550],
    ];

    println!(
        "Generating {} synthetic frames for tokio_tungstenite...",
        target_frames
    );
    for i in 0..target_frames {
        let payload = &payloads[i % payloads.len()];
        // Create a server-to-client frame (unmasked)
        let frame = WsFrame::message(payload.clone().into(), OpCode::Data(Data::Text), true);

        let mut buf = Vec::new();
        frame.format(&mut buf).unwrap();
        frame_data.extend_from_slice(&buf);
    }

    let frame_data_len = frame_data.len();
    println!("Created {} bytes of frame data", frame_data_len);

    // Use tokio::io::duplex to create an in-memory bidirectional pipe
    let (writer, reader) = tokio::io::duplex(1024 * 1024); // 1MB buffer

    // Write all frame data in a separate task
    let write_handle = tokio::spawn(async move {
        let mut writer = writer;
        writer.write_all(&frame_data).await.unwrap();
        writer.shutdown().await.unwrap();
    });

    // Create WebSocket stream from the reader
    let ws_stream = WebSocketStream::from_raw_socket(reader, Role::Client, None).await;
    let (_write, mut read) = ws_stream.split();

    let mut timings = Vec::new();
    let start_time = Instant::now();

    println!(
        "Reading {} frames with tokio_tungstenite (synthetic)...",
        target_frames
    );

    for i in 0..target_frames {
        let frame_start = Instant::now();
        match read.next().await {
            Some(Ok(_message)) => {
                let frame_duration = frame_start.elapsed();
                timings.push(frame_duration);

                if i < 10 || i % 1000 == 0 {
                    println!("Frame #{}: time={:?}", i + 1, frame_duration);
                }
            }
            Some(Err(e)) => {
                eprintln!("Error at frame {}: {}", i + 1, e);
                break;
            }
            None => {
                eprintln!("Stream ended at frame {}", i + 1);
                break;
            }
        }
    }

    // Wait for writer to finish
    let _ = write_handle.await;

    let total_time = start_time.elapsed();
    let avg_time = if !timings.is_empty() {
        timings.iter().sum::<Duration>() / timings.len() as u32
    } else {
        Duration::ZERO
    };
    let min_time = timings.iter().min().copied().unwrap_or(Duration::ZERO);
    let max_time = timings.iter().max().copied().unwrap_or(Duration::ZERO);
    let p50 = if timings.len() > 0 {
        let mut sorted = timings.clone();
        sorted.sort();
        sorted[timings.len() / 2]
    } else {
        Duration::ZERO
    };
    let p99 = if timings.len() > 0 {
        let mut sorted = timings.clone();
        sorted.sort();
        sorted[(timings.len() * 99) / 100]
    } else {
        Duration::ZERO
    };

    println!("\n=== tokio_tungstenite Synthetic Results ===");
    println!("Total frames: {}", timings.len());
    println!("Total time: {:?}", total_time);
    println!("Average time per frame: {:?}", avg_time);
    println!("Min time per frame: {:?}", min_time);
    println!("Max time per frame: {:?}", max_time);
    println!("P50 time per frame: {:?}", p50);
    println!("P99 time per frame: {:?}", p99);
    println!(
        "Frames per second: {:.2}",
        timings.len() as f64 / total_time.as_secs_f64()
    );
    println!(
        "Throughput: {:.2} MB/s",
        (frame_data_len as f64 / 1024.0 / 1024.0) / total_time.as_secs_f64()
    );
}
