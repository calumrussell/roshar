use crate::error::{Error, Result};
use crate::frame::Frame;
use crate::handshake::{create_handshake_request, generate_key, parse_handshake_response};
use crate::reader::{Reader, ReaderType};
use crate::writer::{Writer, WriterType};
use bytes::BytesMut;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::{ClientConfig, ServerName};
use std::sync::OnceLock;

// Platform-specific stream type
#[cfg(feature = "uring")]
type TcpStream = tokio_uring::net::TcpStream;

#[cfg(not(feature = "uring"))]
type TcpStream = tokio::net::TcpStream;

/// WebSocket client using tokio_uring for efficient I/O (on Linux) or regular tokio (on other platforms)
///
/// This client exposes the underlying TcpStream for configuration
/// and provides a stream of frames without copying into tungstenite types.
pub struct Client {
    reader: Reader,
    writer: Writer,
}

impl Client {
    /// Connect to a WebSocket server (plain TCP, ws://)
    ///
    /// # Arguments
    /// * `addr` - The server address (e.g., "127.0.0.1:8080")
    /// * `path` - The WebSocket path (e.g., "/ws")
    /// * `max_frame_size` - Maximum frame size in bytes
    pub async fn connect(addr: &str, path: &str, max_frame_size: usize) -> Result<Self> {
        Self::connect_internal(addr, path, max_frame_size, None).await
    }

    /// Connect to a WebSocket server over TLS (wss://)
    ///
    /// # Arguments
    /// * `addr` - The server address (e.g., "example.com:443")
    /// * `hostname` - The hostname for TLS verification (e.g., "example.com")
    /// * `path` - The WebSocket path (e.g., "/ws")
    /// * `max_frame_size` - Maximum frame size in bytes
    /// * `tls_config` - Optional TLS client configuration. If None, uses default root certificates.
    pub async fn connect_tls(
        addr: &str,
        hostname: &str,
        path: &str,
        max_frame_size: usize,
        tls_config: Option<ClientConfig>,
    ) -> Result<Self> {
        let config = tls_config.map(std::sync::Arc::new).unwrap_or_else(|| {
            // Cache the default config to avoid reloading certificates on every connection
            // This is the REAL optimization - certificate loading happens only once!
            static DEFAULT_CONFIG: OnceLock<std::sync::Arc<ClientConfig>> = OnceLock::new();
            DEFAULT_CONFIG.get_or_init(|| {
                let mut roots = tokio_rustls::rustls::RootCertStore::empty();
                
                // Load native root certificates
                // rustls-native-certs 0.6 returns Result<Vec<Certificate>, io::Error>
                match rustls_native_certs::load_native_certs() {
                    Ok(certs) if !certs.is_empty() => {
                        // Use add_parsable_certificates for better performance (batch operation)
                        // This is more efficient than adding certs one by one
                        let cert_slices: Vec<&[u8]> = certs.iter().map(|c| c.0.as_ref()).collect();
                        let (_number_added, _number_ignored) = roots.add_parsable_certificates(&cert_slices);
                        
                        // Logging removed for performance - can be re-enabled for debugging
                        // if number_added == 0 {
                        //     eprintln!("Warning: No native root certificates were successfully added (ignored {number_ignored}/{total_number})");
                        // } else if number_ignored > 0 {
                        //     eprintln!("Debug: Added {number_added}/{total_number} native root certificates (ignored {number_ignored})");
                        // }
                    }
                    Ok(_) => {
                        // Empty cert list - continue with empty root store
                    }
                    Err(_e) => {
                        // Log error but don't fail - continue with empty root store
                        // Logging removed for performance - can be re-enabled for debugging
                        // eprintln!("Warning: Failed to load native root certificates: {}", e);
                    }
                }
                
                std::sync::Arc::new(
                    ClientConfig::builder()
                        .with_safe_defaults()
                        .with_root_certificates(roots)
                        .with_no_client_auth()
                )
            }).clone()
        });

        let connector = TlsConnector::from(config);
        let server_name = ServerName::try_from(hostname)
            .map_err(|e| Error::Tls(format!("Invalid hostname: {}", e)))?;

        Self::connect_internal(addr, path, max_frame_size, Some((connector, server_name))).await
    }

    async fn connect_internal(
        addr: &str,
        path: &str,
        max_frame_size: usize,
        tls: Option<(TlsConnector, ServerName)>,
    ) -> Result<Self> {
        // Parse address - try as SocketAddr first, otherwise resolve hostname
        let socket_addr = if let Ok(addr) = addr.parse::<std::net::SocketAddr>() {
            addr
        } else {
            // Try to resolve hostname:port
            let (host, port) = addr.rsplit_once(':')
                .ok_or_else(|| Error::Handshake(format!("Invalid address format: {}", addr)))?;
            let port: u16 = port.parse()
                .map_err(|e| Error::Handshake(format!("Invalid port: {}", e)))?;
            
            // Resolve hostname
            use tokio::net::lookup_host;
            let mut addrs = lookup_host((host, port)).await
                .map_err(|e| Error::Handshake(format!("Failed to resolve hostname {}: {}", host, e)))?;
            
            addrs.next()
                .ok_or_else(|| Error::Handshake(format!("No addresses found for {}", host)))?
        };

        // Connect using platform-specific stream
        let tcp_stream = TcpStream::connect(socket_addr).await
            .map_err(|e| Error::Io(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                format!("Failed to connect: {}", e),
            )))?;

        // Perform TLS handshake if needed, then split the stream
        let (reader_type, writer_type, host) = if let Some((connector, server_name)) = tls {
            let server_name_clone = match &server_name {
                ServerName::DnsName(dns_name) => dns_name.as_ref().to_string(),
                ServerName::IpAddress(ip) => ip.to_string(),
                _ => format!("{}:{}", socket_addr.ip(), socket_addr.port()),
            };
            
            // Do TLS handshake on the full stream
            let tls_stream = connector.connect(server_name, tcp_stream).await
                .map_err(|e| Error::Tls(format!("TLS handshake failed: {}", e)))?;
            
            // Split the TLS stream into read and write halves
            #[cfg(feature = "uring")]
            let (tls_read, tls_write) = tokio_uring::io::split(tls_stream);
            #[cfg(not(feature = "uring"))]
            let (tls_read, tls_write) = tokio::io::split(tls_stream);
            (
                ReaderType::Tls(Box::new(tls_read)),
                WriterType::Tls(Box::new(tls_write)),
                server_name_clone,
            )
        } else {
            // Split the plain TCP stream
            #[cfg(feature = "uring")]
            let (tcp_read, tcp_write) = tokio_uring::io::split(tcp_stream);
            #[cfg(not(feature = "uring"))]
            let (tcp_read, tcp_write) = tokio::io::split(tcp_stream);
            (
                ReaderType::Plain(Box::new(tcp_read)),
                WriterType::Plain(Box::new(tcp_write)),
                format!("{}:{}", socket_addr.ip(), socket_addr.port()),
            )
        };

        // Create reader and writer
        let mut reader = Reader::new(reader_type, max_frame_size);
        let mut writer = Writer::new(writer_type);

        // Perform WebSocket handshake
        let key = generate_key();
        let request = create_handshake_request(path, &host, &key);

        // Send handshake request
        writer.write_handshake(&request).await?;

        // Read handshake response - use read_buf for efficient reading
        let mut response = BytesMut::with_capacity(4096);
        
        loop {
            // Use read_buf which is optimized for BytesMut (avoids zeroing)
            let n = reader.read_handshake_buf(&mut response).await?;
            
            if n == 0 {
                return Err(Error::ConnectionClosed);
            }
            
            // Check if we have a complete HTTP response - optimized search
            // Search from the end backwards for \r\n\r\n (HTTP response terminator)
            if response.len() >= 4 {
                // Simple backward search - more efficient than windows().rposition()
                let bytes = response.as_ref();
                let mut found = false;
                for i in (3..bytes.len()).rev() {
                    if bytes[i-3] == b'\r' && bytes[i-2] == b'\n' && 
                       bytes[i-1] == b'\r' && bytes[i] == b'\n' {
                        response.truncate(i + 1);
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
        }

        // Parse and validate handshake response
        parse_handshake_response(&response)?;

        Ok(Client {
            reader,
            writer,
        })
    }

    /// Configure socket options before connecting
    ///
    /// Note: Socket options must be configured on the TcpStream before calling
    /// `connect()` or `connect_tls()`. For TLS connections, configure the underlying
    /// TcpStream before the TLS handshake.
    ///
    /// Example:
    /// ```ignore
    /// use tokio::net::TcpStream;
    /// let stream = TcpStream::connect("127.0.0.1:8080").await?;
    /// stream.set_nodelay(true)?;
    /// // Then use the stream with Client::connect...
    /// ```

    /// Split the client into separate reader and writer
    ///
    /// This allows concurrent reading and writing operations.
    pub fn split(self) -> (Reader, Writer) {
        (self.reader, self.writer)
    }

    /// Send a frame
    ///
    /// This writes the frame directly to the writer without creating an intermediate buffer,
    /// avoiding unnecessary copies of the frame payload.
    pub async fn send_frame(&mut self, frame: &Frame) -> Result<()> {
        self.writer.send_frame(frame).await
    }
}


