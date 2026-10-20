//! A timer that wins a `select!` against the connection must not lose the bytes of a
//! message that is half read.
//!
//! The server writes one framebuffer update in small pieces with a pause after each, as a
//! slow or distant server does. The client races its reads against a timer that fires faster
//! than the pieces arrive, as `screen record` and the viewer do.

use std::time::Duration;

use tokio::io::{duplex, AsyncWriteExt, BufReader, DuplexStream};
use tokio::time::{interval, sleep, timeout};

use testanyware_rfb::{proto::PixelFormat, RfbConnection, ServerEvent};

const SIDE: u16 = 16;
/// Small enough that one update crosses many pieces, large enough to cut every field of it.
const PIECE: usize = 7;
const PAUSE: Duration = Duration::from_millis(2);
const TIMER: Duration = Duration::from_millis(1);

/// The no-auth handshake of a `SIDE`×`SIDE` desktop.
fn handshake_script() -> Vec<u8> {
    let mut script = Vec::new();
    script.extend_from_slice(b"RFB 003.008\n");
    script.extend_from_slice(&[1, 1]);
    script.extend_from_slice(&0u32.to_be_bytes());
    script.extend_from_slice(&SIDE.to_be_bytes());
    script.extend_from_slice(&SIDE.to_be_bytes());
    script.extend_from_slice(&PixelFormat::rgba32_le().encode());
    script.extend_from_slice(&2u32.to_be_bytes());
    script.extend_from_slice(b"vm");
    script
}

/// The BGRX pixel at `index`, distinct from its neighbours so that a lost byte shifts it.
fn pixel(index: usize, round: u8) -> [u8; 4] {
    [index as u8, (index * 3) as u8 ^ round, (index * 7) as u8, 0]
}

/// One Raw update that covers the whole desktop.
fn update(round: u8) -> Vec<u8> {
    let mut msg = vec![0, 0];
    msg.extend_from_slice(&1u16.to_be_bytes());
    for field in [0, 0, SIDE, SIDE] {
        msg.extend_from_slice(&field.to_be_bytes());
    }
    msg.extend_from_slice(&0i32.to_be_bytes());
    for index in 0..usize::from(SIDE) * usize::from(SIDE) {
        msg.extend_from_slice(&pixel(index, round));
    }
    msg
}

/// Write the handshake at once, then each update in pieces. The stream stays open until the
/// task ends, so the client never sees an end of input.
async fn serve(mut server: DuplexStream, rounds: u8) -> DuplexStream {
    server.write_all(&handshake_script()).await.unwrap();
    for round in 0..rounds {
        for piece in update(round).chunks(PIECE) {
            server.write_all(piece).await.unwrap();
            server.flush().await.unwrap();
            sleep(PAUSE).await;
        }
    }
    server
}

#[tokio::test]
async fn a_timer_that_wins_the_race_loses_no_byte_of_a_split_update() {
    const ROUNDS: u8 = 3;
    let (client, server) = duplex(64 * 1024);
    let server = tokio::spawn(serve(server, ROUNDS));
    let mut conn = RfbConnection::handshake(BufReader::new(client), None)
        .await
        .unwrap();

    let mut timer = interval(TIMER);
    let mut timer_wins = 0u32;
    let mut updates = 0u8;
    timeout(Duration::from_secs(20), async {
        while updates < ROUNDS {
            tokio::select! {
                // Only the wait for a first byte is raced: the message is read outside it.
                ready = conn.readable() => {
                    ready.unwrap();
                    match conn.next_message().await.unwrap() {
                        ServerEvent::FramebufferUpdated { rectangles } => {
                            assert_eq!(rectangles, 1);
                            let rgba = conn.framebuffer().rgba().to_vec();
                            for (index, px) in rgba.chunks_exact(4).enumerate() {
                                let [b, g, r, _] = pixel(index, updates);
                                assert_eq!(
                                    px,
                                    &[r, g, b, 0xFF],
                                    "pixel {index} of update {updates}"
                                );
                            }
                            updates += 1;
                        }
                        other => panic!("expected FramebufferUpdated, got {other:?}"),
                    }
                }
                _ = timer.tick() => timer_wins += 1,
            }
        }
    })
    .await
    .expect("every update arrives");

    // The timer must have won races with a half-read message, or the test proves nothing.
    assert!(timer_wins > 20, "the timer won only {timer_wins} races");
    drop(server.await.unwrap());
}
