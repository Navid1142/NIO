use crate::p2p_handshake::Handshake;
use crate::p2p_handshake_session::P2pHandshakeSession;
use crate::p2p_message_codec::P2pMessageCodec;
use crate::p2p_node::P2pNode;
use crate::p2p_protocol::{MessageType, ProtocolMessage, ProtocolSession};
use crate::p2p_transport::P2pConnection;

use std::io;
use std::net::SocketAddr;

#[derive(Debug)]
pub struct P2pRuntime {
    node: P2pNode,
    local_node_id: [u8; 32],
    user_agent: String,
}

impl P2pRuntime {
    pub fn new(
        node: P2pNode,
        local_node_id: [u8; 32],
        user_agent: impl Into<String>,
    ) -> Result<Self, String> {
        if local_node_id == [0u8; 32] {
            return Err("local node id cannot be zero".to_string());
        }

        let user_agent = user_agent.into();

        if user_agent.is_empty() {
            return Err("user agent cannot be empty".to_string());
        }

        if user_agent.len() > 128 {
            return Err("user agent is too long".to_string());
        }

        Ok(Self {
            node,
            local_node_id,
            user_agent,
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.node.address()
    }

    pub fn local_node_id(&self) -> [u8; 32] {
        self.local_node_id
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    pub fn accept_connection(&self) -> io::Result<P2pConnection> {
        self.node.accept_connection()
    }

    pub fn perform_handshake(&self, connection: &P2pConnection) -> io::Result<ProtocolSession> {
        let local_handshake =
            Handshake::new(self.local_node_id, &self.user_agent).map_err(io::Error::other)?;

        let mut handshake_session =
            P2pHandshakeSession::new(local_handshake).map_err(io::Error::other)?;

        handshake_session.perform(connection)?;

        let remote = handshake_session.remote().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "handshake completed without remote peer",
            )
        })?;

        let mut protocol_session =
            ProtocolSession::new(self.local_node_id).map_err(io::Error::other)?;

        protocol_session
            .set_remote_node_id(remote.node_id)
            .map_err(io::Error::other)?;

        Ok(protocol_session)
    }

    pub fn send_message(
        &self,
        connection: &P2pConnection,
        session: &mut ProtocolSession,
        message: &ProtocolMessage,
    ) -> io::Result<()> {
        if !session.is_established() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "protocol session is not established",
            ));
        }

        let mut stream = connection.stream().try_clone()?;

        /*
         * مهم:
         *
         * ابتدا پیام واقعاً از طریق TCP ارسال می‌شود.
         * فقط اگر ارسال موفق بود، sent counter افزایش پیدا می‌کند.
         *
         * بنابراین در صورت شکست TCP،
         * session به اشتباه پیام را sent حساب نمی‌کند.
         */
        P2pMessageCodec::write_message(&mut stream, message)?;

        session.record_sent(message).map_err(io::Error::other)?;

        Ok(())
    }

    pub fn receive_message(
        &self,
        connection: &P2pConnection,
        session: &mut ProtocolSession,
    ) -> io::Result<ProtocolMessage> {
        if !session.is_established() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "protocol session is not established",
            ));
        }

        let mut stream = connection.stream().try_clone()?;

        let message = P2pMessageCodec::read_message(&mut stream)?;

        session
            .record_received(&message)
            .map_err(io::Error::other)?;

        Ok(message)
    }

    pub fn ping(
        &self,
        connection: &P2pConnection,
        session: &mut ProtocolSession,
    ) -> io::Result<()> {
        let message = ProtocolMessage::empty(MessageType::Ping).map_err(io::Error::other)?;

        self.send_message(connection, session, &message)
    }

    pub fn pong(
        &self,
        connection: &P2pConnection,
        session: &mut ProtocolSession,
    ) -> io::Result<()> {
        let message = ProtocolMessage::empty(MessageType::Pong).map_err(io::Error::other)?;

        self.send_message(connection, session, &message)
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

        listener.local_addr().expect("temporary listener address")
    }

    fn node_id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    #[test]
    fn runtime_rejects_zero_node_id() {
        let node = P2pNode::bind(free_address()).expect("node should bind");

        let result = P2pRuntime::new(node, [0u8; 32], "/nio:0.1.0/");

        assert!(result.is_err());
    }

    #[test]
    fn runtime_rejects_empty_user_agent() {
        let node = P2pNode::bind(free_address()).expect("node should bind");

        let result = P2pRuntime::new(node, node_id(1), "");

        assert!(result.is_err());
    }

    #[test]
    fn runtime_rejects_oversized_user_agent() {
        let node = P2pNode::bind(free_address()).expect("node should bind");

        let user_agent = "A".repeat(129);

        let result = P2pRuntime::new(node, node_id(1), user_agent);

        assert!(result.is_err());
    }

    #[test]
    fn runtime_accepts_valid_configuration() {
        let node = P2pNode::bind(free_address()).expect("node should bind");

        let runtime = P2pRuntime::new(node, node_id(1), "/nio:0.1.0/").expect("runtime");

        assert_eq!(runtime.local_node_id(), node_id(1));

        assert_eq!(runtime.user_agent(), "/nio:0.1.0/");
    }

    #[test]
    fn two_runtimes_can_complete_handshake() {
        let server_node = P2pNode::bind(free_address()).expect("server node");

        let address = server_node.local_address().expect("server address");

        let server_runtime =
            P2pRuntime::new(server_node, node_id(1), "/nio:0.1.0/").expect("server runtime");

        let client_node = P2pNode::bind(free_address()).expect("client node");

        let client_runtime =
            P2pRuntime::new(client_node, node_id(2), "/nio:0.1.0/").expect("client runtime");

        let server = thread::spawn(move || {
            let connection = server_runtime.accept_connection().expect("accept");

            let session = server_runtime
                .perform_handshake(&connection)
                .expect("server handshake");

            assert!(session.is_established());

            assert_eq!(session.remote_node_id(), Some(node_id(2)));
        });

        thread::sleep(Duration::from_millis(20));

        let connection = P2pConnection::connect(address).expect("client connect");

        let client_session = client_runtime
            .perform_handshake(&connection)
            .expect("client handshake");

        assert!(client_session.is_established());

        assert_eq!(client_session.remote_node_id(), Some(node_id(1)));

        server.join().expect("server thread");
    }

    #[test]
    fn runtime_rejects_self_connection() {
        let server_node = P2pNode::bind(free_address()).expect("node");

        let address = server_node.local_address().expect("address");

        let server_runtime =
            P2pRuntime::new(server_node, node_id(1), "/nio:0.1.0/").expect("runtime");

        let client_node = P2pNode::bind(free_address()).expect("client node");

        let client_runtime =
            P2pRuntime::new(client_node, node_id(1), "/nio:0.1.0/").expect("runtime");

        let server = thread::spawn(move || {
            let connection = server_runtime.accept_connection().expect("accept");

            assert!(server_runtime.perform_handshake(&connection,).is_err());
        });

        thread::sleep(Duration::from_millis(20));

        let connection = P2pConnection::connect(address).expect("connect");

        assert!(client_runtime.perform_handshake(&connection,).is_err());

        server.join().expect("server thread");
    }

    #[test]
    fn two_runtimes_can_exchange_ping_and_pong() {
        let server_node = P2pNode::bind(free_address()).expect("server node");

        let address = server_node.local_address().expect("server address");

        let server_runtime =
            P2pRuntime::new(server_node, node_id(1), "/nio:0.1.0/").expect("server runtime");

        let client_node = P2pNode::bind(free_address()).expect("client node");

        let client_runtime =
            P2pRuntime::new(client_node, node_id(2), "/nio:0.1.0/").expect("client runtime");

        let server = thread::spawn(move || {
            let connection = server_runtime.accept_connection().expect("accept");

            let mut session = server_runtime
                .perform_handshake(&connection)
                .expect("server handshake");

            let message = server_runtime
                .receive_message(&connection, &mut session)
                .expect("receive ping");

            assert_eq!(message.message_type, MessageType::Ping);

            assert_eq!(session.received_messages(), 1);

            server_runtime
                .pong(&connection, &mut session)
                .expect("send pong");

            assert_eq!(session.sent_messages(), 1);
        });

        thread::sleep(Duration::from_millis(20));

        let connection = P2pConnection::connect(address).expect("client connect");

        let mut client_session = client_runtime
            .perform_handshake(&connection)
            .expect("client handshake");

        client_runtime
            .ping(&connection, &mut client_session)
            .expect("send ping");

        assert_eq!(client_session.sent_messages(), 1);

        let response = client_runtime
            .receive_message(&connection, &mut client_session)
            .expect("receive pong");

        assert_eq!(response.message_type, MessageType::Pong);

        assert_eq!(client_session.received_messages(), 1);

        server.join().expect("server thread");
    }
}
