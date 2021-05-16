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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    SerializationError,
    DeserializationError,
    TlsError,
}

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
    ) -> Result<(), Error> {
        let serialized_msg = match bincode::serialize(msg) {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!("Error serialzing message {:?}: {:?}", msg, e);
                return Err(Error::SerializationError);
            }
        };

        if let Err(e) = self.session.write(&serialized_msg).await {
            tracing::error!("Error writing message {:?} to ServerSession: {:?}", msg, e);
            return Err(Error::TlsError);
        };

        Ok(())
    }

    pub async fn read_message<U: serde::de::DeserializeOwned + std::fmt::Debug>(
        &mut self,
    ) -> Result<U, Error> {
		let out;
      	
       	loop {
			let mut buffer = Vec::with_capacity(4096);
			match self.session.read_buf(&mut buffer).await {
				Err(e) => {
					tracing::error!("Error reading TLS session: {:?}", e);
					return Err(Error::TlsError);
				},
				Ok(0) => continue,
				_ => {
					out = buffer;
					break;
				},
			};
       	}

       let out = match bincode::deserialize::<U>(&out) {
            Ok(deserialized) => deserialized,
            Err(e) => {
                tracing::error!("Error deserializing message: {:?}", e);
                return Err(Error::DeserializationError);
            }
        };
        tracing::trace!("Deserialized message: {:?}", &out);

        Ok(out)
    }
}

