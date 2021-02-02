use async_std::net::{TcpStream, ToSocketAddrs};
use async_std::sync::{Arc, Mutex};
use async_std::task;
use erased_serde as erased;
use futures::channel::oneshot;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::Ordering;

use async_tungstenite::async_std::connect_async;

use crate::codec::{ClientCodec, DefaultCodec};
use crate::error::Error;
use crate::message::{AtomicMessageId, MessageId, RequestHeader, ResponseHeader};
use crate::transport::ws::WebSocketConn;

use crate::server::DEFAULT_RPC_PATH;

/// Type state for creating `Client`
pub struct NotConnected {}
/// Type state for creating `Client`
pub struct Connected {}

type Codec = Arc<Mutex<Box<dyn ClientCodec>>>;
type ResponseBody = Box<dyn erased::Deserializer<'static> + Send>;
type ResponseMap = HashMap<u16, oneshot::Sender<Result<ResponseBody, ResponseBody>>>;

/// RPC Client
pub struct Client<Mode> {
    count: AtomicMessageId,
    inner_codec: Codec,
    pending: Arc<Mutex<ResponseMap>>,

    mode: PhantomData<Mode>,
}

#[cfg(any(
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
))]
/// The following impl block is controlled by feature flag. It is enabled
/// if and only if **exactly one** of the the following feature flag is turned on
/// - `serde_bincode`
/// - `serde_json`
/// - `serde_cbor`
/// - `serde_rmp`
impl Client<NotConnected> {
    /// Connects the an RPC server over socket at the specified network address
    ///
    /// This is enabled
    /// if and only if **exactly one** of the the following feature flag is turned on
    /// - `serde_bincode`
    /// - `serde_json`
    /// - `serde_cbor`
    /// - `serde_rmp`
    ///
    /// Example
    ///
    /// ```rust
    /// use toy_rpc::Client;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     let addr = "127.0.0.1";
    ///     let client = Client::dial(addr).await;
    /// }
    ///
    /// ```
    pub async fn dial(addr: impl ToSocketAddrs) -> Result<Client<Connected>, Error> {
        let stream = TcpStream::connect(addr).await?;

        Ok(Self::with_stream(stream))
    }

    /// Similar to `dial`, this connects to an WebSocket RPC server at the specified network address using the defatul codec
    ///
    /// This is enabled
    /// if and only if **exactly one** of the the following feature flag is turned on
    /// - `serde_bincode`
    /// - `serde_json`
    /// - `serde_cbor`
    /// - `serde_rmp`
    ///
    /// # Example
    ///
    /// ```rust
    /// use toy_rpc::client::Client;
    /// use toy_rpc::error::Error;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     let addr = "ws://127.0.0.1:8888";
    ///     let client = Client::dial_http(addr).await.unwrap();
    /// }
    /// ```
    ///
    pub async fn dial_websocket(addr: &'static str) -> Result<Client<Connected>, Error> {
        let url = url::Url::parse(addr)?;
        Self::_dial_websocket(url).await
    }

    async fn _dial_websocket(url: url::Url) -> Result<Client<Connected>, Error> {
        let (ws_stream, _) = connect_async(&url).await?;
        log::debug!("WebSocket handshake has been successfully completed");

        let ws_stream = WebSocketConn::new(ws_stream);
        let codec = DefaultCodec::with_websocket(ws_stream);

        Ok(Self::with_codec(codec))
    }

    /// Connects to an HTTP RPC server at the specified network address using WebSocket and the defatul codec
    ///
    /// *Warning*: WebSocket is used as the underlying transport protocol starting from version "0.5.0-beta.0",
    /// and this will make client of versions later than "0.5.0-beta.0" incompatible with servers of versions
    /// earlier than "0.5.0-beta.0".
    /// 
    /// If a network path were to be supplpied, the network path must end with a slash "/". Internally, 
    /// `DEFAULT_RPC_PATH="_rpc"` is appended to the end of `addr`, and the rest is the same is calling 
    /// `dial_websocket`.
    ///
    /// This is enabled
    /// if and only if **exactly one** of the the following feature flag is turned on
    /// - `serde_bincode`
    /// - `serde_json`
    /// - `serde_cbor`
    /// - `serde_rmp`
    ///
    /// # Example
    ///
    /// ```rust
    /// use toy_rpc::client::Client;
    /// use toy_rpc::error::Error;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     let addr = "http://127.0.0.1:8888/rpc/";
    ///     let client = Client::dial_http(addr).await.unwrap();
    /// }
    /// ```
    ///
    /// TODO: check if the path ends with a slash
    /// TODO: try send and recv trait object
    pub async fn dial_http(addr: &'static str) -> Result<Client<Connected>, Error> {
        let mut url = url::Url::parse(addr)?.join(DEFAULT_RPC_PATH)?;
        url.set_scheme("ws")
            .expect("Failed to change scheme to ws");
            
        Self::_dial_websocket(url).await
    }

    /// Creates an RPC `Client` over socket with a specified `async_std::net::TcpStream` and the default codec
    ///
    /// This is enabled
    /// if and only if **exactly one** of the the following feature flag is turned on
    /// - `serde_bincode`
    /// - `serde_json`
    /// - `serde_cbor`
    /// - `serde_rmp`
    ///
    /// # Example
    /// ```
    /// use async_std::net::TcpStream;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     let stream = TcpStream::connect("127.0.0.1:8888").await.unwrap();
    ///     let client = Client::with_stream(stream);
    /// }
    /// ```
    pub fn with_stream(stream: TcpStream) -> Client<Connected> {
        let codec = DefaultCodec::new(stream);

        Self::with_codec(codec)
    }
}

impl Client<NotConnected> {
    /// Creates an RPC 'Client` over socket with a specified codec
    ///
    /// Example
    ///
    /// ```rust
    /// use async_std::net::TcpStream;
    /// use toy_rpc::codec::bincode::Codec;
    /// use toy_rpc::Client;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     let addr = "127.0.0.1:8888";
    ///     let stream = TcpStream::connect(addr).await.unwrap();
    ///     let codec = Codec::new(stream);
    ///     let client = Client::with_codec(codec);
    /// }
    /// ```
    pub fn with_codec<C>(codec: C) -> Client<Connected>
    where
        C: ClientCodec + Send + Sync + 'static,
    {
        let box_codec: Box<dyn ClientCodec> = Box::new(codec);

        Client::<Connected> {
            count: AtomicMessageId::new(0u16),
            inner_codec: Arc::new(Mutex::new(box_codec)),
            pending: Arc::new(Mutex::new(HashMap::new())),

            mode: PhantomData,
        }
    }
}

impl Client<Connected> {
    /// Invokes the named function and wait synchronously
    ///
    /// This function internally calls `task::block_on` to wait for the response.
    /// Do NOT use this function inside another `task::block_on`.async_std
    ///
    /// Example
    ///
    /// ```rust
    /// use toy_rpc::client::Client;
    /// use toy_rpc::error::Error;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     let addr = "127.0.0.1:8888";
    ///     let client = Client::dial(addr).await.unwrap();
    ///
    ///     let args = "arguments";
    ///     let reply: Result<String, Error> = client.call("echo_service.echo", &args);
    ///     println!("{:?}", reply);
    /// }
    /// ```
    pub fn call<Req, Res>(&self, service_method: impl ToString, args: Req) -> Result<Res, Error>
    where
        Req: serde::Serialize + Send + Sync,
        Res: serde::de::DeserializeOwned,
    {
        task::block_on(self.async_call(service_method, args))
    }

    /// Invokes the named function asynchronously by spawning a new task and returns the `JoinHandle`
    ///
    /// ```rust
    /// use async_std::task;
    ///
    /// use toy_rpc::client::Client;
    /// use toy_rpc::error::Error;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     let addr = "127.0.0.1:8888";
    ///     let client = Client::dial(addr).await.unwrap();
    ///
    ///     let args = "arguments";
    ///     let handle: task::JoinHandle<Result<Res, Error>> = client.spawn_task("echo_service.echo", args);
    ///     let reply: Result<String, Error> = handle.await;
    ///     println!("{:?}", reply);
    /// }
    /// ```
    pub fn spawn_task<Req, Res>(
        &self,
        service_method: impl ToString + Send + 'static,
        args: Req,
    ) -> task::JoinHandle<Result<Res, Error>>
    where
        Req: serde::Serialize + Send + Sync + 'static,
        Res: serde::de::DeserializeOwned + Send + 'static,
    {
        let codec = self.inner_codec.clone();
        let pending = self.pending.clone();
        let id = self.count.fetch_add(1u16, Ordering::Relaxed);

        task::spawn(
            async move { Self::_async_call(service_method, &args, id, codec, pending).await },
        )
    }

    /// Invokes the named function asynchronously
    ///
    /// Example
    ///
    /// ```rust
    /// use async_std::task;
    ///
    /// use toy_rpc::client::Client;
    /// use toy_rpc::error::Error;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     let addr = "127.0.0.1:8888";
    ///     let client = Client::dial(addr).await.unwrap();
    ///
    ///     let args = "arguments";
    ///     let reply: Result<String, Error> = client.async_call("echo_service.echo", &args).await;
    ///     println!("{:?}", reply);
    /// }
    /// ```
    pub async fn async_call<Req, Res>(
        &self,
        service_method: impl ToString,
        args: Req,
    ) -> Result<Res, Error>
    where
        Req: serde::Serialize + Send + Sync,
        Res: serde::de::DeserializeOwned,
    {
        let codec = self.inner_codec.clone();
        let pending = self.pending.clone();
        let id = self.count.fetch_add(1u16, Ordering::Relaxed);

        Self::_async_call(service_method, &args, id, codec, pending).await
    }

    async fn _async_call<Req, Res>(
        service_method: impl ToString,
        args: &Req,
        id: MessageId,
        codec: Arc<Mutex<Box<dyn ClientCodec>>>,
        pending: Arc<Mutex<ResponseMap>>,
    ) -> Result<Res, Error>
    where
        Req: serde::Serialize + Send + Sync,
        Res: serde::de::DeserializeOwned,
    {
        let _codec = &mut *codec.lock().await;
        let header = RequestHeader {
            id,
            service_method: service_method.to_string(),
        };
        let req = &args as &(dyn erased::Serialize + Send + Sync);

        // send request
        _codec.write_request(header, req).await?;

        // creates channel for receiving response
        let (done_sender, done) = oneshot::channel::<Result<ResponseBody, ResponseBody>>();

        // insert sender to pending map
        {
            let mut _pending = pending.lock().await;
            _pending.insert(id, done_sender);
        }

        Client::<Connected>::_read_response(_codec.as_mut(), pending).await?;

        Client::<Connected>::_handle_response(done, &id)
    }
}

impl Client<Connected> {
    async fn _read_response(
        codec: &mut dyn ClientCodec,
        pending: Arc<Mutex<ResponseMap>>,
    ) -> Result<(), Error> {
        // wait for response
        if let Some(header) = codec.read_response_header().await {
            let ResponseHeader { id, is_error } = header?;
            let deserializer =
                codec
                    .read_response_body()
                    .await
                    .ok_or(Error::IoError(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "Unexpected EOF reading response body",
                    )))?;
            let deserializer = deserializer?;

            let res = match is_error {
                false => Ok(deserializer),
                true => Err(deserializer),
            };

            // send back response
            let mut _pending = pending.lock().await;
            if let Some(done_sender) = _pending.remove(&id) {
                #[cfg(feature = "logging")]
                log::debug!("Sending ResponseBody over oneshot channel {}", &id);
                done_sender.send(res).map_err(|_| Error::TransportError {
                    msg: format!("Failed to send ResponseBody over oneshot channel {}", &id),
                })?;
            }
        }

        Ok(())
    }

    fn _handle_response<Res>(
        mut done: oneshot::Receiver<Result<ResponseBody, ResponseBody>>,
        id: &MessageId,
    ) -> Result<Res, Error>
    where
        Res: serde::de::DeserializeOwned,
    {
        #[cfg(feature = "logging")]
        log::info!("Received response id: {}", &id);

        // wait for result from oneshot channel
        let res = match done.try_recv() {
            Ok(o) => match o {
                Some(r) => r,
                None => {
                    return Err(Error::TransportError {
                        msg: format!("Done channel for id {} is out of date", &id),
                    })
                }
            },
            _ => {
                return Err(Error::TransportError {
                    msg: format!("Done channel for id {} is canceled", &id),
                })
            }
        };

        // deserialize Ok message and Err message
        match res {
            Ok(mut resp_body) => {
                let resp = erased::deserialize(&mut resp_body).map_err(|e| Error::ParseError {
                    source: Box::new(e),
                })?;

                Ok(resp)
            }
            Err(mut err_body) => {
                let err = erased::deserialize(&mut err_body).map_err(|e| Error::ParseError {
                    source: Box::new(e),
                })?;

                Err(Error::RpcError(err))
            }
        }
    }
}