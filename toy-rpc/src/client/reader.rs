use futures::{SinkExt};
use async_trait::async_trait;
use futures::{Sink};
use brw::Running;

use crate::{Error, codec::CodecRead, 
    // message::ResponseHeader
};
use super::broker::ClientBrokerItem;
use crate::protocol::{Header, InboundBody};

pub struct ClientReader<R> {
    pub reader: R,
}

#[async_trait]
impl<R: CodecRead> brw::Reader for ClientReader<R> {
    type BrokerItem = ClientBrokerItem;
    type Ok = ();
    type Error = Error;

    async fn op<B>(&mut self, mut broker: B) -> Running<Result<Self::Ok, Self::Error>>
    where B: Sink<Self::BrokerItem, Error = flume::SendError<Self::BrokerItem>> + Send + Unpin {
        if let Some(header) = self.reader.read_header().await {
            let header: Header = match header {
                Ok(header) => header,
                Err(err) => return Running::Continue(Err(err))
            };
            let deserializer: Box<InboundBody> = match self.reader.read_body().await {
                Some(res) => {
                    match res {
                        Ok(de) => de,
                        Err(err) => return Running::Continue(Err(err))
                    }
                },
                None => return Running::Stop
            };

            match header {
                Header::Response{id, is_ok} => {
                    let res = match is_ok {
                        true => Ok(deserializer),
                        false => Err(deserializer),
                    };

                    if let Err(err) = broker.send(
                        ClientBrokerItem::Response(id, res)
                    ).await {
                        return Running::Continue(Err(err.into()))
                    }
                    Running::Continue(Ok(()))
                },
                _ => {
                    unimplemented!()
                }
            }
        } else {
            if broker.send(
                ClientBrokerItem::Stop
            ).await.is_ok() { }
            Running::Stop
        }
    }
}