use rustls::{ClientConfig, ClientSession, ServerConfig, ServerSession, Session};
use std::{
    io::{Read, Write},
    net::SocketAddr,
    rc::Rc,
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

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
    TcpError,
}

pub struct TlsConnection<T> {
    session: T,
    tcp_stream: TcpStream,
}

impl<T: Session> TlsConnection<T> {
    pub fn new(session: T, tcp_stream: TcpStream) -> TlsConnection<T> {
        TlsConnection {
            session,
            tcp_stream,
        }
    }

    pub fn session(&self) -> &T {
        &self.session
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

        let mut buffer = Vec::new();

        if let Err(e) = self.session.write(&serialized_msg) {
            tracing::error!("Error writing message {:?} to ServerSession: {:?}", msg, e);
            return Err(Error::TlsError);
        };

        if let Err(e) = self.session.write_tls(&mut buffer) {
            tracing::error!(
                "Error encoding message {:?} with ServerSession: {:?}",
                msg,
                e
            );
            return Err(Error::TlsError);
        }

        if let Err(e) = self.tcp_stream.write(&buffer).await {
            tracing::error!(
                "Error sending encoded message {:?} through TCP stream: {:?}",
                msg,
                e
            );
            return Err(Error::TcpError);
        };

        Ok(())
    }

    pub async fn read_message<U: serde::de::DeserializeOwned + std::fmt::Debug>(
        &mut self,
    ) -> Result<Option<U>, Error> {
        let mut buffer = Vec::new();

        if let Err(e) = self.tcp_stream.read_buf(&mut buffer).await {
            tracing::error!(
                "Error receiving encoded messsage through TCP stream: {:?}",
                e
            );
            return Err(Error::TcpError);
        }

        if let Err(e) = self.session.read_tls(&mut buffer.as_slice()) {
            tracing::error!("Error reading message with ServerSession: {:?}", e);
            return Err(Error::TlsError);
        }

        if let Err(e) = self.session.process_new_packets() {
            tracing::error!("Error decoding message with ServerSession: {:?}", e);
            return Err(Error::TlsError);
        }

        if self.session.is_handshaking() {
            return Ok(None);
        }

        let mut buffer = Vec::new();

        if let Err(e) = self.session.read_to_end(&mut buffer) {
            tracing::error!(
                "Error reading message from ServerSession to buffer: {:?}",
                e
            );
            return Err(Error::TlsError);
        }

        let out = match bincode::deserialize::<U>(&mut buffer) {
            Ok(deserialized) => deserialized,
            Err(e) => {
                tracing::error!("Error deserializing message: {:?}", e);
                return Err(Error::DeserializationError);
            }
        };

        Ok(Some(out))
    }
}
