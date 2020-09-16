use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicU16;

// use crate::error::{Error, RpcError};

pub type MessageId = u16;
pub type AtomicMessageId = AtomicU16;

pub trait Metadata {
    fn get_id(&self) -> MessageId;
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct RequestHeader {
    pub id: MessageId,
    pub service_method: String,
}

impl Metadata for RequestHeader {
    fn get_id(&self) -> MessageId {
        self.id.clone()
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ResponseHeader {
    pub id: MessageId,
    pub is_error: bool,
}

impl Metadata for ResponseHeader {
    fn get_id(&self) -> MessageId {
        self.id.clone()
    }
}
