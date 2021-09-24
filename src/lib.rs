use anyhow::Result;
use rustls::{ClientConfig, ClientSession, ServerConfig, ServerSession, Session};
use std::io::{Cursor, Read, Write};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::TlsStream;

#[derive(Debug)]
pub struct TlsConnection<T> {
    session: TlsStream<T>,
    buffer: std::io::Cursor<Vec<u8>>,
}

impl<T: AsyncRead + AsyncWrite + Unpin> TlsConnection<T> {
    pub fn new(session: impl Into<TlsStream<T>>) -> TlsConnection<T> {
        TlsConnection {
            session: session.into(),
            buffer: Cursor::new(Vec::new()),
        }
    }

    pub fn session(&self) -> (&T, &dyn Session) {
        self.session.get_ref()
    }

    pub fn session_mut(&mut self) -> (&mut T, &mut dyn Session) {
        self.session.get_mut()
    }

    pub async fn send_message<U: serde::Serialize + std::fmt::Debug>(
        &mut self,
        msg: &U,
    ) -> Result<()> {
        let serialized_msg = bincode::serialize(msg)?;
        self.session.write_all(&serialized_msg).await?;
        self.session.flush().await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn read_message<U: serde::de::DeserializeOwned + std::fmt::Debug>(
        &mut self,
    ) -> Result<U> {
        // Check the buffer for existing messages before adding to the end
        match bincode::deserialize_from::<_, U>(&mut self.buffer) {
            Ok(msg) => Ok(msg),
            Err(e) => match *e {
                // If it's an EOF, poll for new messages on the socket
                bincode::ErrorKind::Io(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    loop {
                        let mut buffer = Vec::with_capacity(4096);
                        match self.session.read_buf(&mut buffer).await? {
                            0 => {
                                anyhow::bail!("Reader closed");
                            }
                            _ => {
                                self.buffer.get_mut().append(&mut buffer);
                                break;
                            }
                        };
                    }

                    let out = bincode::deserialize_from::<_, U>(&mut self.buffer)?;
                    tracing::trace!("Deserialized message: {:?}", &out);

                    Ok(out)
                }

                e => Err(e)?,
            },
        }
    }

    pub async fn read_message_and_take_self<U: serde::de::DeserializeOwned + std::fmt::Debug>(
        mut self,
    ) -> Result<(U, Self)> {
        Ok((self.read_message().await?, self))
    }
}
