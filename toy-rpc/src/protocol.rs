//! Message protocol between server and client
use std::time::Duration;
use serde::{Serialize, Deserialize};

use crate::message::{MessageId, Metadata};

/// Header of a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Header {
    /// Header of a request
    /// 
    /// The body contains the content of the request
    Request{
        /// Message id
        id: MessageId,
        /// RPC service and method in the format of "{Service}.{method}"
        service_method: String,
        /// RPC timeout, all requests will have timeouts
        timeout: Duration,
    },

    /// Header of a response
    ///
    /// The body contains the content of the response/result
    Response{
        /// Message id
        id: MessageId,
        /// Whether the result is Ok
        is_ok: bool,
    },

    /// Header of a cancellation message
    ///
    /// TODO: The body should be an unit type ie. `()`
    Cancel(MessageId),

    /// Header of a publish message
    ///
    /// The body contains the publishing content
    Publish {
        /// Message id
        id: MessageId,
        /// Topic to publish to
        topic: String,
    },

    /// Header of a subscribe message
    /// Message will be pushed to the subscriber
    ///
    /// The body should be an unit type ie. `()`
    Subscribe {
        /// Message id
        id: MessageId,
        /// Topic to subscribe to
        topic: String,
    },

    /// Header of a unsubscribe message
    ///
    /// The body should be an unit type ie. `()`
    Unsubscribe {
        /// Message Id
        id: MessageId,
        /// Topic to unsubscribe from
        topic: String,
    },

    // /// Header of a subscription item
    // ///
    // /// The body contains the content of the subscription item
    // Subscription {
    //     /// Message id
    //     id: MessageId,
    //     /// Topic of the subscription item
    //     topic: String,
    // },

    /// Acknowledge of the following type of messages
    /// - Cancel?
    /// - Publish
    /// - Subscribe
    ///
    /// The body should be an unit type `()`
    Ack(MessageId),

    /// Reserved for a potential message queue like design
    /// Produce a message to be consumed
    Produce {
        /// Message id
        id: MessageId,
        /// Topic of the queue
        topic: String,
        /// Number of times this message can be consumed
        tickets: u32,
    },

    /// Reserved for a potential message queue like design
    /// Consumes a message by pulling message from broker/server
    Consume {
        /// Message id
        id: MessageId,
        /// Topic of the queue
        topic: String,
    },

    /// Reserved for further extension to the message protocol
    Ext {
        /// Message id
        id: MessageId,
        /// Reserved for content of extension
        content: String,
        /// Reserved for some numerical/enum content
        marker: u32,
    }
}

impl Metadata for Header {
    fn get_id(&self) -> MessageId {
        match self {
            Self::Request{id, ..} => id.clone(),
            Self::Response{id, ..} => id.clone(),
            Self::Cancel(id) => id.clone(),
            Self::Publish {id, ..} => id.clone(),
            Self::Subscribe {id, ..} => id.clone(),
            Self::Unsubscribe {id, .. } => id.clone(),
            // Self::Subscription { id, .. } => id.clone(),
            Self::Ack(id) => id.clone(),
            Self::Produce {id, ..} => id.clone(),
            Self::Consume {id, ..} => id.clone(),
            Self::Ext {id, ..} => id.clone()
        }
    }
}

pub(crate) type OutboundBody = dyn erased_serde::Serialize + Send + Sync;
pub(crate) type InboundBody = dyn erased_serde::Deserializer<'static> + Send;

// pub(crate) struct InboundMessage {
//     header: Header,
//     body: InboundBody
// }

// pub(crate) struct OutboundMessage {
//     header: Header,
//     body: OutboundBody,
// }

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use bincode::{self, Options};
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum MyEnum {
        One(u16),
        Two(String)
    }

    #[test]
    fn size_of_header() {
        let bincode_opt = bincode::DefaultOptions::new()
            .with_varint_encoding();

        let header = Header::Request{
            id: 3000,
            service_method: "".into(),
            timeout: Duration::from_secs(10)
        };
        let size = bincode_opt.serialized_size(&header).unwrap();
        println!("Header::Request size: {:?}", size);

        let header = Header::Response{
            id: 0,
            is_ok: true
        };
        let size = bincode_opt.serialized_size(&header).unwrap();
        println!("Header::Response size: {:?}", size);

        let header = Header::Cancel(0);
        let size = bincode_opt.serialized_size(&header).unwrap();
        println!("Header::Cancel size: {:?}", size);

        let header = Header::Publish{
            id: 0,
            topic: "".into()
        };
        let size = bincode_opt.serialized_size(&header).unwrap();
        println!("Header::Publish size: {:?}", size);

        let header = Header::Subscribe{
            id: 0,
            topic: "".into()
        };
        let size = bincode_opt.serialized_size(&header).unwrap();
        println!("Header::Subscribe size: {:?}", size);

        let header = Header::Ack(0);
        let size = bincode_opt.serialized_size(&header).unwrap();
        println!("Header::Ack size: {:?}", size);

        let opt = MyEnum::Two("".into());
        let size = bincode_opt.serialized_size(&opt).unwrap();
        println!("size: {:?}", size);
    }

    #[test]
    fn bincode_i16() {
        let opt = bincode::DefaultOptions::new()
            .with_varint_encoding();
        let num = 10;
        type Out = i16;
        let num = num as Out;
        let buf = opt.serialize(&num).unwrap();

        // serde::deserialize will yield the correct result
        let mut de = bincode::Deserializer::with_reader(
            Cursor::new(buf.clone()), 
            opt
            );
        let out: Out = serde::Deserialize::deserialize(&mut de).unwrap();
        println!("serde::Deserialize::deserialize: {:?}", out);

        // erased serde somehow always multiplies the result by 2
        let mut de = bincode::Deserializer::with_reader(
            Cursor::new(buf), 
            opt
            );
        let mut de = Box::new(<dyn erased_serde::Deserializer>::erase(&mut de)) as Box<dyn erased_serde::Deserializer>;
        let out: Out = erased_serde::deserialize(&mut de).unwrap();
        println!("erased_serde::deserialize: {:?}", out);
    }
}