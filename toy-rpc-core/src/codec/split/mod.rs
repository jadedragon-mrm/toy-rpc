//! Implements `SplittableServerCodec` and `SplittableClientCodec`

use async_trait::async_trait;
use std::marker::PhantomData;

use super::*;

mod server;
pub use server::*;

mod client;
pub use client::*;

/// Read half of the codec
pub struct CodecReadHalf<R, C, CT> {
    /// The wrapped reader
    pub reader: R,
    /// Marker of the Codec type
    pub marker: PhantomData<C>,
    /// Type of the connection
    pub conn_type: PhantomData<CT>,
}

/// Write half of the codec
#[allow(dead_code)]
pub struct CodecWriteHalf<W, C, CT> {
    /// The wrapped writer
    pub writer: W,
    /// Marker of the Codec type
    pub marker: PhantomData<C>,
    /// Type of the connection
    pub conn_type: PhantomData<CT>,
}

impl<W, C, CT> Marshal for CodecWriteHalf<W, C, CT>
where
    C: Marshal,
{
    fn marshal<S: serde::Serialize>(val: &S) -> Result<Vec<u8>, Error> {
        C::marshal(val)
    }
}

impl<R, C, CT> Unmarshal for CodecReadHalf<R, C, CT>
where
    C: Unmarshal,
{
    fn unmarshal<'de, D: serde::Deserialize<'de>>(buf: &'de [u8]) -> Result<D, Error> {
        C::unmarshal(buf)
    }
}

impl<R, C, CT> EraseDeserializer for CodecReadHalf<R, C, CT>
where
    C: EraseDeserializer,
{
    fn from_bytes(buf: Vec<u8>) -> Box<dyn erased::Deserializer<'static> + Send> {
        C::from_bytes(buf)
    }
}

cfg_if! {
    if #[cfg(all(
        any(feature = "async-std", feature = "tokio"),
        any(
            all(
                feature = "serde_bincode",
                not(feature = "serde_json"),
                not(feature = "serde_cbor"),
                not(feature = "serde_rmp"),
            ),
            all(
                feature = "serde_cbor",
                not(feature = "serde_json"),
                not(feature = "serde_bincode"),
                not(feature = "serde_rmp"),
            ),
            all(
                feature = "serde_rmp",
                not(feature = "serde_cbor"),
                not(feature = "serde_json"),
                not(feature = "serde_bincode"),
            )
        )
    ))] {
        use crate::transport::frame::{Frame, PayloadType, FrameRead, FrameWrite};

        #[async_trait]
        impl<R, C> CodecRead for CodecReadHalf<R, C, ConnTypeReadWrite>
        where
            R: FrameRead + Send + Unpin,
            C: Unmarshal + EraseDeserializer + Send
        {
            async fn read_header<H>(&mut self) -> Option<Result<H, Error>>
            where
                H: serde::de::DeserializeOwned,
            {
                let reader = &mut self.reader;

                Some(
                    reader
                        .read_frame()
                        .await?
                        .and_then(|frame| Self::unmarshal(&frame.payload)),
                )
            }

            async fn read_body(
                &mut self,
            ) -> Option<Result<RequestDeserializer, Error>> {
                let reader = &mut self.reader;

                match reader.read_frame().await? {
                    Ok(frame) => {
                        let de = C::from_bytes(frame.payload);
                        Some(Ok(de))
                    }
                    Err(e) => return Some(Err(e)),
                }
            }
        }

        #[async_trait]
        impl<W, C> CodecWrite for CodecWriteHalf<W, C, ConnTypeReadWrite>
        where
            W: FrameWrite + Send + Unpin,
            C: Marshal + Send,
        {
            async fn write_header<H>(&mut self, header: H) -> Result<(), Error>
            where
                H: serde::Serialize + Metadata + Send,
            {
                let writer = &mut self.writer;

                let id = header.get_id();
                let buf = Self::marshal(&header)?;
                let frame = Frame::new(id, 0, PayloadType::Header, buf);

                writer.write_frame(frame).await
            }

            async fn write_body(
                &mut self,
                id: &MessageId,
                body: &(dyn erased::Serialize + Send + Sync),
            ) -> Result<(), Error> {
                let writer = &mut self.writer;
                let buf = Self::marshal(&body)?;
                let frame = Frame::new(id.to_owned(), 1, PayloadType::Data, buf.to_owned());
                writer.write_frame(frame).await
            }
        }
    }
}

cfg_if! {
    if #[cfg(all(
        any(
            feature = "async-std",
            feature = "tokio",
        ),
        any(
            all(
                feature = "serde_bincode",
                not(feature = "serde_json"),
                not(feature = "serde_cbor"),
                not(feature = "serde_rmp"),
            ),
            all(
                feature = "serde_cbor",
                not(feature = "serde_json"),
                not(feature = "serde_bincode"),
                not(feature = "serde_rmp"),
            ),
            all(
                feature = "serde_json",
                not(feature = "serde_bincode"),
                not(feature = "serde_cbor"),
                not(feature = "serde_rmp"),
            ),
            all(
                feature = "serde_rmp",
                not(feature = "serde_cbor"),
                not(feature = "serde_json"),
                not(feature = "serde_bincode"),
            )
        )
    ))] {
        use crate::transport::{PayloadRead, PayloadWrite};
        use crate::util::GracefulShutdown;

        #[async_trait]
        impl<R, C> CodecRead for CodecReadHalf<R, C, ConnTypePayload>
        where
            R: PayloadRead + Send,
            C: Unmarshal + EraseDeserializer + Send
        {
            async fn read_header<H>(&mut self) -> Option<Result<H, Error>>
            where
                H: serde::de::DeserializeOwned,
            {
                let reader = &mut self.reader;

                Some(
                    reader
                        .read_payload()
                        .await?
                        .and_then(|payload| Self::unmarshal(&payload)),
                )
            }

            async fn read_body(
                &mut self,
            ) -> Option<Result<RequestDeserializer, Error>> {
                let reader = &mut self.reader;

                match reader.read_payload().await? {
                    Ok(payload) => {
                        let de = Self::from_bytes(payload);
                        Some(Ok(de))
                    }
                    Err(e) => return Some(Err(e)),
                }
            }
        }

        #[async_trait]
        impl<W, C> CodecWrite for CodecWriteHalf<W, C, ConnTypePayload>
        where
            W: PayloadWrite + Send,
            C: Marshal + Send,
        {
            async fn write_header<H>(&mut self, header: H) -> Result<(), Error>
            where
                H: serde::Serialize + Metadata + Send,
            {
                let writer = &mut self.writer;
                let buf = Self::marshal(&header)?;
                writer.write_payload(buf).await
            }

            async fn write_body(
                &mut self,
                _: &MessageId,
                body: &(dyn erased::Serialize + Send + Sync),
            ) -> Result<(), Error> {
                let buf = Self::marshal(&body)?;
                let writer = &mut self.writer;
                writer.write_payload(buf).await
            }
        }

        #[async_trait]
        impl<W, C, Conn> GracefulShutdown for CodecWriteHalf<W, C, Conn>
        where
            W: GracefulShutdown + Send,
            C: Send,
            Conn: Send,
        {
            async fn close(&mut self) {
                self.writer.close().await;
            }
        }
    }
}
