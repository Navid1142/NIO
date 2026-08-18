use crate::p2p::PeerId;

pub const HANDSHAKE_VERSION: u32 = 1;
pub const MAX_USER_AGENT_LENGTH: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    pub protocol_version: u32,
    pub node_id: PeerId,
    pub user_agent: String,
}

impl Handshake {
    pub fn new(node_id: PeerId, user_agent: impl Into<String>) -> Result<Self, String> {
        let user_agent = user_agent.into();

        if user_agent.is_empty() {
            return Err("user agent cannot be empty".to_string());
        }

        if user_agent.len() > MAX_USER_AGENT_LENGTH {
            return Err("user agent is too long".to_string());
        }

        Ok(Self {
            protocol_version: HANDSHAKE_VERSION,
            node_id,
            user_agent,
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.protocol_version != HANDSHAKE_VERSION {
            return Err("unsupported handshake protocol version".to_string());
        }

        if self.node_id == [0u8; 32] {
            return Err("node id cannot be zero".to_string());
        }

        if self.user_agent.is_empty() {
            return Err("user agent cannot be empty".to_string());
        }

        if self.user_agent.len() > MAX_USER_AGENT_LENGTH {
            return Err("user agent is too long".to_string());
        }

        Ok(())
    }

    pub fn is_compatible(&self, local_version: u32) -> bool {
        self.protocol_version == local_version
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandshakeState {
    NotStarted,
    Sent,
    Received,
    Established,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeSession {
    local: Handshake,
    remote: Option<Handshake>,
    state: HandshakeState,
}

impl HandshakeSession {
    pub fn new(local: Handshake) -> Result<Self, String> {
        local.validate()?;

        Ok(Self {
            local,
            remote: None,
            state: HandshakeState::NotStarted,
        })
    }

    pub fn local(&self) -> &Handshake {
        &self.local
    }

    pub fn remote(&self) -> Option<&Handshake> {
        self.remote.as_ref()
    }

    pub fn state(&self) -> &HandshakeState {
        &self.state
    }

    pub fn mark_sent(&mut self) -> Result<(), String> {
        match self.state {
            HandshakeState::NotStarted => {
                self.state = HandshakeState::Sent;
                Ok(())
            }

            HandshakeState::Sent => Err("handshake has already been sent".to_string()),

            HandshakeState::Received => {
                Err("cannot mark sent after receiving handshake".to_string())
            }

            HandshakeState::Established => Err("handshake is already established".to_string()),

            HandshakeState::Failed => Err("handshake session has failed".to_string()),
        }
    }

    pub fn receive(&mut self, remote: Handshake) -> Result<(), String> {
        // A failed session can never recover.
        if self.state == HandshakeState::Failed {
            return Err("handshake session has failed".to_string());
        }

        // Reject an already-established session.
        if self.state == HandshakeState::Established {
            return Err("handshake is already established".to_string());
        }

        // Validate the remote handshake fields first.
        //
        // IMPORTANT:
        // If the remote protocol version is incompatible, the session
        // must enter Failed state even when validate() rejects it.
        if remote.protocol_version != self.local.protocol_version {
            self.state = HandshakeState::Failed;
            self.remote = None;

            return Err("incompatible handshake protocol version".to_string());
        }

        // Now validate the rest of the handshake.
        if let Err(error) = remote.validate() {
            self.state = HandshakeState::Failed;
            self.remote = None;

            return Err(error);
        }

        // Reject self-connections.
        if remote.node_id == self.local.node_id {
            self.state = HandshakeState::Failed;
            self.remote = None;

            return Err("self connection is not allowed".to_string());
        }

        // A second received handshake is not allowed.
        if self.state == HandshakeState::Received {
            return Err("duplicate handshake received".to_string());
        }

        self.remote = Some(remote);

        match self.state {
            HandshakeState::Sent => {
                self.state = HandshakeState::Established;
            }

            HandshakeState::NotStarted => {
                self.state = HandshakeState::Received;
            }

            HandshakeState::Received => {
                return Err("duplicate handshake received".to_string());
            }

            HandshakeState::Established => {
                return Err("handshake is already established".to_string());
            }

            HandshakeState::Failed => unreachable!(),
        }

        Ok(())
    }

    pub fn is_established(&self) -> bool {
        self.state == HandshakeState::Established
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node_id(value: u8) -> PeerId {
        [value; 32]
    }

    #[test]
    fn valid_handshake_is_created() {
        let handshake =
            Handshake::new(node_id(1), "/nio:0.1.0/").expect("handshake should be created");

        assert_eq!(handshake.protocol_version, HANDSHAKE_VERSION);
        assert_eq!(handshake.node_id, node_id(1));
        assert_eq!(handshake.user_agent, "/nio:0.1.0/");
        assert!(handshake.validate().is_ok());
    }

    #[test]
    fn zero_node_id_is_rejected() {
        let handshake =
            Handshake::new([0u8; 32], "/nio:0.1.0/").expect("construction itself is valid");

        assert!(handshake.validate().is_err());
    }

    #[test]
    fn empty_user_agent_is_rejected() {
        assert!((Handshake::new(node_id(1), "")).is_err());
    }

    #[test]
    fn oversized_user_agent_is_rejected() {
        let value = "x".repeat(MAX_USER_AGENT_LENGTH + 1);

        assert!((Handshake::new(node_id(1), value)).is_err());
    }

    #[test]
    fn protocol_version_is_one() {
        assert_eq!(HANDSHAKE_VERSION, 1);
    }

    #[test]
    fn compatible_versions_are_accepted() {
        let handshake = Handshake::new(node_id(1), "/nio:0.1.0/").expect("handshake");

        assert!(handshake.is_compatible(HANDSHAKE_VERSION));
    }

    #[test]
    fn incompatible_versions_are_rejected() {
        let handshake = Handshake::new(node_id(1), "/nio:0.1.0/").expect("handshake");

        assert!(!handshake.is_compatible(HANDSHAKE_VERSION + 1));
    }

    #[test]
    fn session_starts_not_started() {
        let handshake = Handshake::new(node_id(1), "/nio:0.1.0/").expect("handshake");

        let session = HandshakeSession::new(handshake).expect("session");

        assert_eq!(session.state(), &HandshakeState::NotStarted);

        assert!(!session.is_established());
    }

    #[test]
    fn sending_handshake_changes_state() {
        let handshake = Handshake::new(node_id(1), "/nio:0.1.0/").expect("handshake");

        let mut session = HandshakeSession::new(handshake).expect("session");

        session.mark_sent().expect("send should succeed");

        assert_eq!(session.state(), &HandshakeState::Sent);
    }

    #[test]
    fn valid_remote_handshake_establishes_session() {
        let local = Handshake::new(node_id(1), "/nio:0.1.0/").expect("local");

        let remote = Handshake::new(node_id(2), "/nio:0.1.0/").expect("remote");

        let mut session = HandshakeSession::new(local).expect("session");

        session.mark_sent().expect("send");

        session.receive(remote).expect("receive");

        assert!(session.is_established());

        assert_eq!(session.remote().unwrap().node_id, node_id(2));
    }

    #[test]
    fn receiving_before_sending_is_allowed_but_not_established() {
        let local = Handshake::new(node_id(1), "/nio:0.1.0/").expect("local");

        let remote = Handshake::new(node_id(2), "/nio:0.1.0/").expect("remote");

        let mut session = HandshakeSession::new(local).expect("session");

        session.receive(remote).expect("receive");

        assert_eq!(session.state(), &HandshakeState::Received);

        assert!(!session.is_established());
    }

    #[test]
    fn self_connection_is_rejected() {
        let local = Handshake::new(node_id(1), "/nio:0.1.0/").expect("local");

        let remote = Handshake::new(node_id(1), "/nio:0.1.0/").expect("remote");

        let mut session = HandshakeSession::new(local).expect("session");

        session.mark_sent().expect("send");

        assert!(session.receive(remote).is_err());

        assert_eq!(session.state(), &HandshakeState::Failed);
    }

    #[test]
    fn incompatible_remote_version_is_rejected() {
        let local = Handshake::new(node_id(1), "/nio:0.1.0/").expect("local");

        let mut remote = Handshake::new(node_id(2), "/nio:0.1.0/").expect("remote");

        remote.protocol_version = HANDSHAKE_VERSION + 1;

        let mut session = HandshakeSession::new(local).expect("session");

        session.mark_sent().expect("send");

        assert!(session.receive(remote).is_err());

        assert_eq!(session.state(), &HandshakeState::Failed);

        assert!(session.remote().is_none());
    }

    #[test]
    fn duplicate_send_is_rejected() {
        let local = Handshake::new(node_id(1), "/nio:0.1.0/").expect("local");

        let mut session = HandshakeSession::new(local).expect("session");

        session.mark_sent().expect("first send");

        assert!(session.mark_sent().is_err());
    }

    #[test]
    fn duplicate_receive_is_rejected() {
        let local = Handshake::new(node_id(1), "/nio:0.1.0/").expect("local");

        let remote = Handshake::new(node_id(2), "/nio:0.1.0/").expect("remote");

        let mut session = HandshakeSession::new(local).expect("session");

        session.receive(remote.clone()).expect("first receive");

        assert!(session.receive(remote).is_err());
    }
}
