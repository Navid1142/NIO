use crate::p2p_handshake::{Handshake, HandshakeSession, HandshakeState, MAX_USER_AGENT_LENGTH};
use crate::p2p_transport::P2pConnection;
use std::io::{self, Read, Write};

const HANDSHAKE_MAGIC: [u8; 4] = *b"NIOH";
const MAX_HANDSHAKE_FRAME_SIZE: usize = 4 + 4 + 32 + 2 + MAX_USER_AGENT_LENGTH;

#[derive(Debug)]
pub struct P2pHandshakeSession {
    session: HandshakeSession,
}

impl P2pHandshakeSession {
    pub fn new(local: Handshake) -> Result<Self, String> {
        Ok(Self {
            session: HandshakeSession::new(local)?,
        })
    }

    pub fn session(&self) -> &HandshakeSession {
        &self.session
    }

    pub fn state(&self) -> &HandshakeState {
        self.session.state()
    }

    pub fn is_established(&self) -> bool {
        self.session.is_established()
    }

    pub fn remote(&self) -> Option<&Handshake> {
        self.session.remote()
    }

    pub fn perform(&mut self, connection: &P2pConnection) -> io::Result<()> {
        self.session.mark_sent().map_err(io::Error::other)?;

        write_handshake(connection.stream(), self.session.local())?;

        let remote = read_handshake(connection.stream())?;

        self.session.receive(remote).map_err(io::Error::other)?;

        Ok(())
    }
}

fn write_handshake(mut stream: &std::net::TcpStream, handshake: &Handshake) -> io::Result<()> {
    handshake.validate().map_err(io::Error::other)?;

    let user_agent = handshake.user_agent.as_bytes();

    if user_agent.len() > MAX_USER_AGENT_LENGTH {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "user agent is too long",
        ));
    }

    let user_agent_length = u16::try_from(user_agent.len()).map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "user agent length exceeds u16")
    })?;

    let frame_length = 4 + 32 + 2 + user_agent.len();

    if frame_length > MAX_HANDSHAKE_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "handshake frame is too large",
        ));
    }

    let frame_length_u32 = u32::try_from(frame_length).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "handshake frame length exceeds u32",
        )
    })?;

    let mut buffer = Vec::with_capacity(8 + frame_length);

    buffer.extend_from_slice(&HANDSHAKE_MAGIC);

    buffer.extend_from_slice(&frame_length_u32.to_be_bytes());

    buffer.extend_from_slice(&handshake.protocol_version.to_be_bytes());

    buffer.extend_from_slice(&handshake.node_id);

    buffer.extend_from_slice(&user_agent_length.to_be_bytes());

    buffer.extend_from_slice(user_agent);

    stream.write_all(&buffer)?;
    stream.flush()?;

    Ok(())
}

fn read_handshake(mut stream: &std::net::TcpStream) -> io::Result<Handshake> {
    let mut magic = [0u8; 4];

    stream.read_exact(&mut magic)?;

    if magic != HANDSHAKE_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid handshake magic",
        ));
    }

    let mut length_bytes = [0u8; 4];

    stream.read_exact(&mut length_bytes)?;

    let frame_length = u32::from_be_bytes(length_bytes) as usize;

    if frame_length > MAX_HANDSHAKE_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "handshake frame is too large",
        ));
    }

    if frame_length < 4 + 32 + 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "handshake frame is too small",
        ));
    }

    let mut frame = vec![0u8; frame_length];

    stream.read_exact(&mut frame)?;

    let mut cursor = 0usize;

    let protocol_version = read_u32(&frame, &mut cursor)?;

    let node_id = read_array_32(&frame, &mut cursor)?;

    let user_agent_length = read_u16(&frame, &mut cursor)? as usize;

    if user_agent_length > MAX_USER_AGENT_LENGTH {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "user agent is too long",
        ));
    }

    let remaining = frame.len().saturating_sub(cursor);

    if remaining != user_agent_length {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid user agent length",
        ));
    }

    let user_agent_bytes = &frame[cursor..cursor + user_agent_length];

    let user_agent = String::from_utf8(user_agent_bytes.to_vec())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "user agent is not valid utf8"))?;

    let handshake = Handshake {
        protocol_version,
        node_id,
        user_agent,
    };

    handshake.validate().map_err(io::Error::other)?;

    Ok(handshake)
}

fn read_u16(buffer: &[u8], cursor: &mut usize) -> io::Result<u16> {
    if buffer.len().saturating_sub(*cursor) < 2 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "truncated u16",
        ));
    }

    let bytes = [buffer[*cursor], buffer[*cursor + 1]];

    *cursor += 2;

    Ok(u16::from_be_bytes(bytes))
}

fn read_u32(buffer: &[u8], cursor: &mut usize) -> io::Result<u32> {
    if buffer.len().saturating_sub(*cursor) < 4 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "truncated u32",
        ));
    }

    let bytes = [
        buffer[*cursor],
        buffer[*cursor + 1],
        buffer[*cursor + 2],
        buffer[*cursor + 3],
    ];

    *cursor += 4;

    Ok(u32::from_be_bytes(bytes))
}

fn read_array_32(buffer: &[u8], cursor: &mut usize) -> io::Result<[u8; 32]> {
    if buffer.len().saturating_sub(*cursor) < 32 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "truncated node id",
        ));
    }

    let mut result = [0u8; 32];

    result.copy_from_slice(&buffer[*cursor..*cursor + 32]);

    *cursor += 32;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p_node::P2pNode;
    use std::net::{SocketAddr, TcpListener};
    use std::thread;
    use std::time::Duration;

    fn node_id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn free_address() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").expect("temporary listener should bind");

        listener.local_addr().expect("temporary listener address")
    }

    #[test]
    fn two_nodes_can_complete_real_tcp_handshake() {
        let address = free_address();

        let server_node = P2pNode::bind(address).expect("server node should bind");

        let actual_address = server_node.local_address().expect("server address");

        let server = server_node.try_clone().expect("server should clone");

        let server_thread = thread::spawn(move || {
            let connection = server.accept_connection().expect("server should accept");

            let local = Handshake::new(node_id(1), "/nio:0.1.0/").expect("server handshake");

            let mut session = P2pHandshakeSession::new(local).expect("server session");

            session
                .perform(&connection)
                .expect("server handshake should succeed");

            assert!(session.is_established());

            assert_eq!(session.remote().unwrap().node_id, node_id(2));
        });

        thread::sleep(Duration::from_millis(20));

        let connection = P2pConnection::connect(actual_address).expect("client should connect");

        let local = Handshake::new(node_id(2), "/nio:0.1.0/").expect("client handshake");

        let mut session = P2pHandshakeSession::new(local).expect("client session");

        session
            .perform(&connection)
            .expect("client handshake should succeed");

        assert!(session.is_established());

        assert_eq!(session.remote().unwrap().node_id, node_id(1));

        server_thread.join().expect("server thread should finish");
    }

    #[test]
    fn self_connection_is_rejected_over_tcp() {
        let address = free_address();

        let server_node = P2pNode::bind(address).expect("node should bind");

        let actual_address = server_node.local_address().expect("node address");

        let server = server_node.try_clone().expect("node should clone");

        let server_thread = thread::spawn(move || {
            let connection = server.accept_connection().expect("server should accept");

            let local = Handshake::new(node_id(1), "/nio:0.1.0/").expect("handshake");

            let mut session = P2pHandshakeSession::new(local).expect("session");

            assert!(session.perform(&connection).is_err());

            assert_eq!(session.state(), &HandshakeState::Failed);
        });

        thread::sleep(Duration::from_millis(20));

        let connection = P2pConnection::connect(actual_address).expect("client should connect");

        let local = Handshake::new(node_id(1), "/nio:0.1.0/").expect("handshake");

        let mut session = P2pHandshakeSession::new(local).expect("session");

        assert!(session.perform(&connection).is_err());

        server_thread.join().expect("server thread should finish");
    }

    #[test]
    fn invalid_magic_is_rejected() {
        let address = free_address();

        let listener = TcpListener::bind(address).expect("listener should bind");

        let actual = listener.local_addr().expect("listener address");

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");

            stream.write_all(b"BAD!").expect("write invalid magic");
        });

        thread::sleep(Duration::from_millis(20));

        let connection = P2pConnection::connect(actual).expect("client should connect");

        let result = read_handshake(connection.stream());

        assert!(result.is_err());

        server.join().expect("server thread should finish");
    }
}
