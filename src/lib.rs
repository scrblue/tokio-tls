use anyhow::Result;
use rustls::{ClientConfig, ClientSession, ServerConfig, ServerSession, Session};
use std::{
    io::{Read, Write},
    net::SocketAddr,
    rc::Rc,
    sync::Arc,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::TlsStream;


pub struct TlsConnection<T> {
    session: TlsStream<T>,
}

impl<T: AsyncRead + AsyncWrite + Unpin> TlsConnection<T> {
    pub fn new(session: impl Into<TlsStream<T>>) -> TlsConnection<T> {
        TlsConnection {
            session: session.into(),
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
        self.session.write(&serialized_msg).await?;

        Ok(())
    }

    pub async fn read_message<U: serde::de::DeserializeOwned + std::fmt::Debug>(
        &mut self,
    ) -> Result<U> {
		let out;
      	
       	loop {
			let mut buffer = Vec::with_capacity(4096);
			match self.session.read_buf(&mut buffer).await? {
				0 => {
    				anyhow::bail!("Reader closed");
					let _ = tokio::task::yield_now().await;
					break;
				},
				_ => {
					out = buffer;
					break;
				},
			};
       }

       let out = bincode::deserialize::<U>(&out)?;
       tracing::trace!("Deserialized message: {:?}", &out);

        Ok(out)
    }
}

