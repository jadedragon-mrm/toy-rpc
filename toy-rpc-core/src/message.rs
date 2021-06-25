//! Custom definition of rpc request and response headers
use cfg_if::cfg_if;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicU16;
use std::time::Duration;

/// Type of message id is u16
pub type MessageId = u16;

/// Atomic type of MessageId
pub type AtomicMessageId = AtomicU16;

/// Returning the metadata
pub trait Metadata {
    /// Gets the id from the metadata
    fn get_id(&self) -> MessageId;
}

/// Header of a request
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct RequestHeader {
    /// The id number of a request.
    ///
    /// The id number is tracked by the client and monotonically increases.
    pub id: MessageId,

    /// A string that represents the requested service and method
    ///
    /// The format should follow "{service}.{method}" where {service} should be
    /// replaced by the service name and {method} should be replaced by the method name.
    /// Both the service name and method name are case sensitive.
    pub service_method: String,
}

impl Metadata for RequestHeader {
    fn get_id(&self) -> MessageId {
        self.id
    }
}

/// Header of a response
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ResponseHeader {
    /// The id of an RPC response.
    ///
    /// The response will have the same id as the request
    pub id: MessageId,

    /// Whether the response carries an error message
    pub is_error: bool,
}

impl Metadata for ResponseHeader {
    fn get_id(&self) -> MessageId {
        self.id
    }
}

/// The Error message that will be sent over for a error response
#[derive(Serialize, Deserialize)]
pub enum ErrorMessage {
    InvalidArgument,
    ServiceNotFound,
    MethodNotFound,
    ExecutionError(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TimeoutRequestBody(pub Duration);

#[cfg(any(
    all(feature = "async-std", not(feature = "tokio")),
    all(feature = "tokio", not(feature = "async-std"))
))]
#[cfg(any(feature = "client"))]
impl TimeoutRequestBody {
    pub fn new(dur: Duration) -> Self {
        Self(dur)
    }
}

/// Client request needs to be serialzed while the request on the server needs to be
/// deserialized
#[cfg(feature = "client")]
pub type ClientRequestBody = Box<dyn erased_serde::Serialize + Send + Sync>;
#[cfg(feature = "client")]
pub type ClientResponseBody = Box<dyn erased_serde::Deserializer<'static> + Send>;
/// The serialized representation of the response body
#[cfg(feature = "client")]
pub type ClientResponseResult = Result<ClientResponseBody, ClientResponseBody>;

cfg_if! {
    if #[cfg(any(
        feature = "async-std",
        feature = "tokio"
    ))] {
        #[cfg(any(feature = "server", feature = "client"))]
        pub const CANCELLATION_TOKEN: &str = "RPC_TASK_CANCELLATION";
        #[cfg(any(feature = "server", feature = "client"))]
        pub const CANCELLATION_TOKEN_DELIM: &str = ".";

        #[cfg(any(feature = "server", feature = "client"))]
        pub const TIMEOUT_TOKEN: &str = "RPC_TASK_TIMEOUT";

        #[cfg(feature = "server")]
        use crate::{
            error::Error,
            codec::RequestDeserializer,
            service::{ArcAsyncServiceCall, HandlerResult},
        };

        #[cfg(feature = "server")]
        impl ErrorMessage {
            pub fn from_err(err: Error) -> Result<Self, Error> {
                match err {
                    Error::InvalidArgument => Ok(Self::InvalidArgument),
                    Error::ServiceNotFound => Ok(Self::ServiceNotFound),
                    Error::MethodNotFound => Ok(Self::MethodNotFound),
                    Error::ExecutionError(s) => Ok(Self::ExecutionError(s)),
                    e @ Error::IoError(_) => Err(e),
                    e @ Error::ParseError(_) => Err(e),
                    e @ Error::Internal(_) => Err(e),
                    e @ Error::Canceled(_) => Err(e),
                    e @ Error::Timeout(_) => Err(e),
                }
            }
        }

        #[cfg(feature = "server")]
        #[cfg_attr(feature = "actix", derive(actix::Message))]
        #[cfg_attr(feature = "actix", rtype(result = "()"))]
        pub enum ExecutionMessage {
            TimeoutInfo(MessageId, Duration),
            Request {
                call: ArcAsyncServiceCall,
                id: MessageId,
                method: String,
                deserializer: RequestDeserializer,
            },
            Result(ExecutionResult),
            Cancel(MessageId),
            Stop,
        }

        #[cfg(feature = "server")]
        #[cfg_attr(feature = "actix", derive(actix::Message))]
        #[cfg_attr(feature = "actix", rtype(result = "()"))]
        pub struct ExecutionResult {
            pub id: MessageId,
            pub result: HandlerResult,
        }

        #[cfg(feature = "server")]
        pub enum RequestType {
            Timeout(MessageId),
            Request {
                id: MessageId,
                service: String,
                method: String,
            },
            Cancel(MessageId),
        }
    }
}