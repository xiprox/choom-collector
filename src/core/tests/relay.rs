//! Stage 3's exit, against a real relay: run `npm run dev` in relay/, then
//! `cargo test -p coop-core --test relay -- --ignored`. COOP_TEST_RELAY names another relay.

use std::time::Duration;

use coop_core::relay::{HostEvent, HostLink, TesterEvent, TesterLink, new_code};
use tokio::time::timeout;

fn relay() -> String {
    std::env::var("COOP_TEST_RELAY").unwrap_or_else(|_| "ws://127.0.0.1:8787".into())
}

const WAIT: Duration = Duration::from_secs(5);

async fn host_event(host: &mut HostLink) -> HostEvent {
    timeout(WAIT, host.next()).await.expect("the host should hear something within 5s")
}

async fn tester_event(tester: &mut TesterLink) -> TesterEvent {
    timeout(WAIT, tester.next()).await.expect("the tester should hear something within 5s")
}

#[tokio::test]
#[ignore = "needs a relay: npm run dev in relay/"]
async fn a_host_and_two_testers_exchange_messages() {
    let code = new_code();
    let mut host = HostLink::open(&relay(), &code, None).await.unwrap();
    let mut first = TesterLink::open(&relay(), &code).await.unwrap();
    assert_eq!(host_event(&mut host).await, HostEvent::TesterJoined(first.id));
    let mut second = TesterLink::open(&relay(), &code).await.unwrap();
    assert_eq!(host_event(&mut host).await, HostEvent::TesterJoined(second.id));

    first.send(b"logs from the first").await.unwrap();
    assert_eq!(host_event(&mut host).await, HostEvent::Message { from: first.id, data: b"logs from the first".to_vec() });

    host.send_to(&second.id, b"for the second only").await.unwrap();
    host.send_to_everyone(b"for everyone").await.unwrap();
    assert_eq!(tester_event(&mut second).await, TesterEvent::Message(b"for the second only".to_vec()));
    assert_eq!(tester_event(&mut second).await, TesterEvent::Message(b"for everyone".to_vec()));
    assert_eq!(tester_event(&mut first).await, TesterEvent::Message(b"for everyone".to_vec()));

    let first_id = first.id;
    first.leave().await;
    assert_eq!(host_event(&mut host).await, HostEvent::TesterLeft(first_id));

    host.stop().await;
    assert_eq!(tester_event(&mut second).await, TesterEvent::Ended("host-stopped".into()));
}

#[tokio::test]
#[ignore = "needs a relay: npm run dev in relay/"]
async fn a_host_that_returns_within_30s_keeps_its_testers() {
    let code = new_code();
    let host = HostLink::open(&relay(), &code, None).await.unwrap();
    let resume = host.resume.clone();
    let mut tester = TesterLink::open(&relay(), &code).await.unwrap();

    // Dropping the link without closing it is what a lost connection looks like to the relay.
    drop(host);
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Without the token the code can't be taken over, even while its host is away.
    assert!(HostLink::open(&relay(), &code, None).await.is_err());

    let mut host = HostLink::open(&relay(), &code, Some(&resume)).await.unwrap();
    host.send_to_everyone(b"still here").await.unwrap();
    assert_eq!(tester_event(&mut tester).await, TesterEvent::Message(b"still here".to_vec()));

    tester.send(b"and so am I").await.unwrap();
    assert_eq!(host_event(&mut host).await, HostEvent::Message { from: tester.id, data: b"and so am I".to_vec() });
    host.stop().await;
}

#[tokio::test]
#[ignore = "needs a relay: npm run dev in relay/"]
async fn joining_a_code_nobody_hosts_says_so() {
    let error = TesterLink::open(&relay(), &new_code()).await.err().expect("joining should fail");
    assert!(error.to_string().contains("unknown-code"), "{error:#}");
}
