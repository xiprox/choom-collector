//! The relay client: one WebSocket to the host's relay, as the host or as a tester
//! (record/areas/network/01-relay-and-sessions.md). The relay's own frames are JSON text; players'
//! messages travel as binary frames, whose routing bytes this module adds and strips.

use anyhow::{Context as _, Result, anyhow, bail};
use futures_util::{SinkExt as _, StreamExt as _};
use serde::Deserialize;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

// relay/src/protocol.ts holds the same values, so change both together.
pub const CODE_ALPHABET: &str = "23456789ABCDEFGHJKMNPQRSTVWXYZ";
pub const CODE_LENGTH: usize = 6;
const CLOSE_INTENTIONAL: u16 = 4001;
const TO_EVERYONE: u8 = 0;
const TO_ONE: u8 = 1;
const ID_BYTES: usize = 16;

/// A tester's ID, which the relay gives each tester's socket: a UUID, as its 16 bytes.
pub type TesterId = [u8; ID_BYTES];

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// A code picked at random, as the host's PC picks its own once and keeps it.
pub fn new_code() -> String {
    let alphabet = CODE_ALPHABET.as_bytes();
    (0..CODE_LENGTH).map(|_| alphabet[rand::random_range(0..alphabet.len())] as char).collect()
}

/// Takes a code as players type it, with a hyphen or spaces, in either case.
pub fn parse_code(input: &str) -> Option<String> {
    let cleaned: String = input.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_uppercase()).collect();
    (cleaned.len() == CODE_LENGTH && cleaned.chars().all(|c| CODE_ALPHABET.contains(c))).then_some(cleaned)
}

/// The WebSocket address for what a player enters: `relay.example.net` is `wss://relay.example.net`,
/// and an address that names its scheme, such as `ws://127.0.0.1:8787` for a relay on this PC,
/// keeps it.
pub fn socket_address(relay: &str) -> String {
    let relay = relay.trim().trim_end_matches('/');
    if let Some(rest) = relay.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = relay.strip_prefix("http://") {
        format!("ws://{rest}")
    } else if relay.starts_with("wss://") || relay.starts_with("ws://") {
        relay.to_string()
    } else {
        format!("wss://{relay}")
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum Control {
    Hosting { resume: String },
    Joined {
        #[serde(rename = "guestId")]
        guest_id: String,
    },
    GuestJoined {
        #[serde(rename = "guestId")]
        guest_id: String,
    },
    GuestLeft {
        #[serde(rename = "guestId")]
        guest_id: String,
    },
    Closed { reason: String },
}

fn parse_id(text: &str) -> Result<TesterId> {
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    if hex.len() != ID_BYTES * 2 {
        bail!("the relay sent a tester ID that isn't a UUID: {text:?}");
    }
    let mut id = [0u8; ID_BYTES];
    for (i, byte) in id.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).with_context(|| format!("the tester ID {text:?} isn't hex"))?;
    }
    Ok(id)
}

#[derive(Debug, PartialEq, Eq)]
pub enum HostEvent {
    TesterJoined(TesterId),
    TesterLeft(TesterId),
    Message { from: TesterId, data: Vec<u8> },
    /// The socket closed; reconnecting with the resume token within 30s keeps the testers.
    Dropped,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TesterEvent {
    Message(Vec<u8>),
    /// The relay ended the session, such as `host-stopped` or `host-left`.
    Ended(String),
    /// The socket closed without the relay saying why.
    Dropped,
}

/// The host's socket to its relay.
pub struct HostLink {
    socket: Socket,
    /// What proves to the relay, after a drop, that this is the same host.
    pub resume: String,
}

impl HostLink {
    /// Opens the session under `code`, or takes it back with the resume token after a drop.
    pub async fn open(relay: &str, code: &str, resume: Option<&str>) -> Result<Self> {
        let mut address = format!("{}/host?code={code}", socket_address(relay));
        if let Some(resume) = resume {
            address.push_str(&format!("&resume={resume}"));
        }
        let (mut socket, _) = connect_async(&address).await.map_err(|e| refused(e, "host"))?;
        match next_control(&mut socket).await? {
            Control::Hosting { resume } => Ok(Self { socket, resume }),
            _ => bail!("the relay answered hosting with something other than a session"),
        }
    }

    pub async fn send_to_everyone(&mut self, message: &[u8]) -> Result<()> {
        let mut frame = Vec::with_capacity(1 + message.len());
        frame.push(TO_EVERYONE);
        frame.extend_from_slice(message);
        self.socket.send(Message::binary(frame)).await?;
        Ok(())
    }

    pub async fn send_to(&mut self, tester: &TesterId, message: &[u8]) -> Result<()> {
        let mut frame = Vec::with_capacity(1 + ID_BYTES + message.len());
        frame.push(TO_ONE);
        frame.extend_from_slice(tester);
        frame.extend_from_slice(message);
        self.socket.send(Message::binary(frame)).await?;
        Ok(())
    }

    pub async fn next(&mut self) -> HostEvent {
        while let Some(Ok(message)) = self.socket.next().await {
            match message {
                Message::Binary(frame) if frame.len() >= ID_BYTES => {
                    let mut from = [0u8; ID_BYTES];
                    from.copy_from_slice(&frame[..ID_BYTES]);
                    return HostEvent::Message { from, data: frame[ID_BYTES..].to_vec() };
                }
                Message::Text(text) => match serde_json::from_str::<Control>(&text) {
                    Ok(Control::GuestJoined { guest_id }) => {
                        if let Ok(id) = parse_id(&guest_id) {
                            return HostEvent::TesterJoined(id);
                        }
                    }
                    Ok(Control::GuestLeft { guest_id }) => {
                        if let Ok(id) = parse_id(&guest_id) {
                            return HostEvent::TesterLeft(id);
                        }
                    }
                    _ => {}
                },
                Message::Close(_) => break,
                _ => {}
            }
        }
        HostEvent::Dropped
    }

    /// Ends the session at once, where a dropped socket would keep it for 30s.
    pub async fn stop(mut self) {
        let frame = CloseFrame { code: CloseCode::from(CLOSE_INTENTIONAL), reason: "host-stopped".into() };
        let _ = self.socket.close(Some(frame)).await;
    }
}

/// A tester's socket to the host's relay.
pub struct TesterLink {
    socket: Socket,
    pub id: TesterId,
}

impl TesterLink {
    pub async fn open(relay: &str, code: &str) -> Result<Self> {
        let address = format!("{}/join?code={code}", socket_address(relay));
        let (mut socket, _) = connect_async(&address).await.map_err(|e| refused(e, "join"))?;
        match next_control(&mut socket).await? {
            Control::Joined { guest_id } => Ok(Self { socket, id: parse_id(&guest_id)? }),
            _ => bail!("the relay answered joining with something other than a place in the session"),
        }
    }

    pub async fn send(&mut self, message: &[u8]) -> Result<()> {
        self.socket.send(Message::binary(message.to_vec())).await?;
        Ok(())
    }

    pub async fn next(&mut self) -> TesterEvent {
        while let Some(Ok(message)) = self.socket.next().await {
            match message {
                Message::Binary(data) => return TesterEvent::Message(data.to_vec()),
                Message::Text(text) => {
                    if let Ok(Control::Closed { reason }) = serde_json::from_str::<Control>(&text) {
                        return TesterEvent::Ended(reason);
                    }
                }
                Message::Close(Some(frame)) if !frame.reason.is_empty() => return TesterEvent::Ended(frame.reason.to_string()),
                Message::Close(_) => break,
                _ => {}
            }
        }
        TesterEvent::Dropped
    }

    pub async fn leave(mut self) {
        let _ = self.socket.close(None).await;
    }
}

/// Reads the relay's first frame on a socket: its answer to `/host` or `/join`.
async fn next_control(socket: &mut Socket) -> Result<Control> {
    loop {
        match socket.next().await {
            Some(Ok(Message::Text(text))) => return serde_json::from_str(&text).context("the relay sent a frame this app doesn't know"),
            // The relay refuses a tester by closing the socket with the reason, such as
            // `session-full` or `unknown-code`, since a WebSocket client can't read a status code.
            Some(Ok(Message::Close(frame))) => {
                let reason = frame.map(|f| f.reason.to_string()).unwrap_or_default();
                bail!("the relay closed the connection: {}", if reason.is_empty() { "no reason given" } else { &reason })
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(e.into()),
            None => bail!("the relay closed the connection before answering"),
        }
    }
}

fn refused(error: tungstenite::Error, what: &str) -> anyhow::Error {
    match error {
        tungstenite::Error::Http(response) => match response.status().as_u16() {
            409 => anyhow!("another host is using this code"),
            403 => anyhow!("another host holds this code"),
            400 => anyhow!("the relay says the code is malformed"),
            429 => anyhow!("the relay is limiting attempts from this network; try again in a minute"),
            status => anyhow!("the relay refused to {what}: HTTP {status}"),
        },
        other => anyhow!(other).context(format!("couldn't reach the relay to {what}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_six_characters_from_the_alphabet() {
        for _ in 0..100 {
            let code = new_code();
            assert_eq!(parse_code(&code), Some(code.clone()));
        }
    }

    #[test]
    fn a_typed_code_loses_its_hyphen_and_case() {
        assert_eq!(parse_code("k7r-m2q").as_deref(), Some("K7RM2Q"));
        assert_eq!(parse_code("K7R M2Q").as_deref(), Some("K7RM2Q"));
        assert_eq!(parse_code("K7R-M2"), None);
        // O and 0 aren't in the alphabet, since they're misread for each other.
        assert_eq!(parse_code("K7R-M20"), None);
    }

    #[test]
    fn a_relay_address_without_a_scheme_is_secure() {
        assert_eq!(socket_address("relay.example.net"), "wss://relay.example.net");
        assert_eq!(socket_address("https://relay.example.net/"), "wss://relay.example.net");
        assert_eq!(socket_address("ws://127.0.0.1:8787"), "ws://127.0.0.1:8787");
    }

    #[test]
    fn tester_ids_read_from_uuids() {
        let id = parse_id("0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0").unwrap();
        assert_eq!(id[0], 0x0f);
        assert_eq!(id[15], 0xf0);
        assert!(parse_id("not-a-uuid").is_err());
    }
}
