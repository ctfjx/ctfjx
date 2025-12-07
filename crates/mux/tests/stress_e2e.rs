use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

mod util;
use util::make_mux_pair;

const N_STREAMS: usize = 1 << 20;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stress_many_streams_single_conn() {
    let (client_mux, server_mux) = make_mux_pair();

    // channel to collect server streams
    let (server_done_tx, mut server_done_rx) = mpsc::unbounded_channel();

    // spawn server accept loop
    let server_handle = tokio::spawn(async move {
        for _ in 0..N_STREAMS {
            let mut server_stream = server_mux.accept().await.expect("server accept failed");

            let server_done_tx = server_done_tx.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 32];
                let n = server_stream.read(&mut buf).await.expect("read failed");
                buf.truncate(n);

                // echo back to client
                server_stream
                    .write_all(&buf)
                    .await
                    .expect("write back failed");
                server_stream.flush().await.expect("flush failed");
                server_done_tx.send(buf).expect("send to channel failed");
            });
        }
    });

    // spawn N client streams
    for i in 0..N_STREAMS {
        let mut client_stream = client_mux.open().await.expect("client open failed");
        let msg = format!("hello server stream {i}").into_bytes();

        tokio::spawn(async move {
            client_stream
                .write_all(&msg)
                .await
                .expect("client write failed");
            client_stream.flush().await.expect("client flush failed");

            let mut buf = vec![0u8; msg.len()];
            client_stream
                .read_exact(&mut buf)
                .await
                .expect("client read failed");

            assert_eq!(buf, msg, "mismatch on stream {i}");
        });
    }

    // wait for all server responses
    for _ in 0..N_STREAMS {
        let _ = server_done_rx
            .recv()
            .await
            .expect("server never sent response");
    }

    // close multiplexers
    client_mux.close();
    server_handle.await.expect("server task failed");

    println!("All streams completed successfully!");
}
