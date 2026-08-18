use crate::p2p_transport::{P2pConnection, P2pListener};
use std::io;
use std::net::SocketAddr;

#[derive(Debug)]
pub struct P2pNode {
    listener: P2pListener,
    address: SocketAddr,
}

impl P2pNode {
    pub fn bind(address: SocketAddr) -> io::Result<Self> {
        let listener = P2pListener::bind(address)?;
        let actual_address = listener.local_address()?;

        Ok(Self {
            listener,
            address: actual_address,
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn local_address(&self) -> io::Result<SocketAddr> {
        self.listener.local_address()
    }

    pub fn accept_connection(&self) -> io::Result<P2pConnection> {
        self.listener.accept()
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        let listener = self.listener.try_clone()?;

        Ok(Self {
            listener,
            address: self.address,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    fn free_address() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").expect("temporary listener should bind");

        listener
            .local_addr()
            .expect("temporary listener should have address")
    }

    #[test]
    fn node_can_bind() {
        let address = free_address();

        let node = P2pNode::bind(address).expect("node should bind");

        assert_eq!(node.address().ip(), address.ip());
    }

    #[test]
    fn node_reports_local_address() {
        let address = free_address();

        let node = P2pNode::bind(address).expect("node should bind");

        let local = node.local_address().expect("local address should exist");

        assert_eq!(local.ip(), address.ip());
    }

    #[test]
    fn node_accepts_connection() {
        let address = free_address();

        let node = P2pNode::bind(address).expect("node should bind");

        let actual_address = node.local_address().expect("node address");

        let server = node.try_clone().expect("node should clone");

        let handle = thread::spawn(move || {
            server
                .accept_connection()
                .expect("node should accept connection")
                .peer_address()
        });

        thread::sleep(Duration::from_millis(20));

        let client = std::net::TcpStream::connect(actual_address).expect("client should connect");

        let peer_address = handle.join().expect("server thread should finish");

        assert_eq!(peer_address, client.local_addr().unwrap());
    }

    #[test]
    fn node_can_be_cloned() {
        let address = free_address();

        let node = P2pNode::bind(address).expect("node should bind");

        let clone = node.try_clone().expect("node should clone");

        assert_eq!(node.address(), clone.address());

        assert_eq!(
            node.local_address().unwrap(),
            clone.local_address().unwrap()
        );
    }
}
