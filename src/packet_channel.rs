use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub fn async_channel<S: Serialize + Send + 'static, R: for<'a> Deserialize<'a> + Send + 'static>(
    stream: TcpStream,
) -> (
    tokio::sync::mpsc::Receiver<Result<R>>,
    tokio::sync::mpsc::Sender<S>,
) {
    let (in_tx, in_rx) = tokio::sync::mpsc::channel(1024);
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel(1024);

    tokio::spawn(async move {
        let mut stream = stream;

        loop {
            let mut data = vec![0; 1024];
            tokio::select! {
                len = stream.read(&mut data) => {
                    if let Ok(len) = len {
                        if len == 0 {
                            in_tx.send(Err(anyhow!("EOF"))).await.unwrap();
                            break;
                        }

                        let data = &data[..len];
                        let Ok(data) = bincode::deserialize::<R>(data) else {
                            in_tx.send(Err(anyhow!("failed to decode packet"))).await.unwrap();
                            break;
                        };
                        in_tx.send(Ok(data)).await.unwrap();
                    } else {
                        in_tx.send(Err(anyhow!("Error"))).await.unwrap();
                        break;
                    }

                }
                data = out_rx.recv() => {
                    if let Some(data) = data {
                        let data = bincode::serialize(&data).unwrap();
                        stream.write_all(&data).await.unwrap();
                    } else {
                        let _ = in_tx.send(Err(anyhow!("Error"))).await;
                        break;
                    }
                }
            };
        }
    });
    (in_rx, out_tx)
}
