use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

const CONNECT_TIMEOUT_SECS: u64 = 10;
const READ_TIMEOUT_SECS: u64 = 30;
const WRITE_TIMEOUT_SECS: u64 = 30;

#[derive(Debug)]
pub struct P2pConnection {
    stream: TcpStream,
    peer_address: SocketAddr,
}

impl P2pConnection {
    pub fn connect(address: SocketAddr) -> io::Result<Self> {
        let timeout = Duration::from_secs(CONNECT_TIMEOUT_SECS);

        let stream = TcpStream::connect_timeout(&address, timeout)?;

        stream.set_read_timeout(Some(Duration::from_secs(READ_TIMEOUT_SECS)))?;

        stream.set_write_timeout(Some(Duration::from_secs(WRITE_TIMEOUT_SECS)))?;

        stream.set_nodelay(true)?;

        Ok(Self {
            stream,
            peer_address: address,
        })
    }

    pub fn from_stream(stream: TcpStream, peer_address: SocketAddr) -> io::Result<Self> {
        stream.set_read_timeout(Some(Duration::from_secs(READ_TIMEOUT_SECS)))?;

        stream.set_write_timeout(Some(Duration::from_secs(WRITE_TIMEOUT_SECS)))?;

        stream.set_nodelay(true)?;

        Ok(Self {
            stream,
            peer_address,
        })
    }

    pub fn peer_address(&self) -> SocketAddr {
        self.peer_address
    }

    pub fn local_address(&self) -> io::Result<SocketAddr> {
        self.stream.local_addr()
    }

    pub fn peer_socket_address(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }

    pub fn shutdown(&self) -> io::Result<()> {
        self.stream.shutdown(std::net::Shutdown::Both)
    }

    pub fn stream(&self) -> &TcpStream {
        &self.stream
    }

    pub fn into_stream(self) -> TcpStream {
        self.stream
    }
}

#[derive(Debug)]
pub struct P2pListener {
    listener: TcpListener,
}

impl P2pListener {
    pub fn bind(address: SocketAddr) -> io::Result<Self> {
        let listener = TcpListener::bind(address)?;

        listener.set_nonblocking(false)?;

        Ok(Self { listener })
    }

    pub fn local_address(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    pub fn accept(&self) -> io::Result<P2pConnection> {
        let (stream, peer_address) = self.listener.accept()?;

        P2pConnection::from_stream(stream, peer_address)
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            listener: self.listener.try_clone()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportConfig {
    pub connect_timeout_secs: u64,
    pub read_timeout_secs: u64,
    pub write_timeout_secs: u64,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout_secs: CONNECT_TIMEOUT_SECS,
            read_timeout_secs: READ_TIMEOUT_SECS,
            write_timeout_secs: WRITE_TIMEOUT_SECS,
        }
    }
}

impl TransportConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.connect_timeout_secs == 0 {
            return Err("connect timeout cannot be zero".to_string());
        }

        if self.read_timeout_secs == 0 {
            return Err("read timeout cannot be zero".to_string());
        }

        if self.write_timeout_secs == 0 {
            return Err("write timeout cannot be zero".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    fn free_address() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind temporary listener");

        listener.local_addr().expect("temporary listener address")
    }

    #[test]
    fn default_transport_config_is_valid() {
        let config = TransportConfig::default();

        assert!(config.validate().is_ok());
        assert!(config.connect_timeout_secs > 0);
        assert!(config.read_timeout_secs > 0);
        assert!(config.write_timeout_secs > 0);
    }

    #[test]
    fn zero_connect_timeout_is_rejected() {
        let config = TransportConfig {
            connect_timeout_secs: 0,
            ..TransportConfig::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn zero_read_timeout_is_rejected() {
        let config = TransportConfig {
            read_timeout_secs: 0,
            ..TransportConfig::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn zero_write_timeout_is_rejected() {
        let config = TransportConfig {
            write_timeout_secs: 0,
            ..TransportConfig::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn listener_can_bind_to_local_address() {
        let address = free_address();

        let listener = P2pListener::bind(address).expect("listener should bind");

        let local = listener
            .local_address()
            .expect("listener should have local address");

        assert_eq!(local.ip(), address.ip());
    }

    #[test]
    fn listener_accepts_tcp_connection() {
        let address = free_address();

        let listener = P2pListener::bind(address).expect("listener should bind");

        let actual_address = listener.local_address().expect("listener address");

        let handle = thread::spawn(move || {
            let connection = listener.accept().expect("connection should be accepted");

            connection.peer_address()
        });

        thread::sleep(Duration::from_millis(20));

        let client = TcpStream::connect(actual_address).expect("client should connect");

        client
            .set_nodelay(true)
            .expect("nodelay should be supported");

        let peer_address = handle.join().expect("accept thread should finish");

        assert_eq!(peer_address, client.local_addr().unwrap());
    }

    #[test]
    fn p2p_connection_can_connect() {
        let address = free_address();

        let listener = P2pListener::bind(address).expect("listener should bind");

        let actual_address = listener.local_address().expect("listener address");

        let handle =
            thread::spawn(move || listener.accept().expect("server should accept connection"));

        thread::sleep(Duration::from_millis(20));

        let connection = P2pConnection::connect(actual_address).expect("client should connect");

        assert_eq!(connection.peer_address(), actual_address);

        let server_connection = handle.join().expect("server thread should finish");

        assert_eq!(
            server_connection.peer_address(),
            connection.local_address().expect("client local address")
        );
    }

    #[test]
    fn connection_has_socket_addresses() {
        let address = free_address();

        let listener = P2pListener::bind(address).expect("listener should bind");

        let actual_address = listener.local_address().expect("listener address");

        let handle =
            thread::spawn(move || listener.accept().expect("server should accept connection"));

        thread::sleep(Duration::from_millis(20));

        let connection = P2pConnection::connect(actual_address).expect("client should connect");

        assert!(connection.local_address().is_ok());
        assert!(connection.peer_socket_address().is_ok());

        let server_connection = handle.join().expect("server thread should finish");

        assert!(server_connection.local_address().is_ok());

        assert!(server_connection.peer_socket_address().is_ok());
    }

    #[test]
    fn listener_can_be_cloned() {
        let address = free_address();

        let listener = P2pListener::bind(address).expect("listener should bind");

        let clone = listener.try_clone().expect("listener should clone");

        assert_eq!(
            listener.local_address().unwrap(),
            clone.local_address().unwrap()
        );
    }
}
