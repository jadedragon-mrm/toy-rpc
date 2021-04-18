/// This module implements integration with `actix-web`.
use cfg_if::cfg_if;
use std::marker::PhantomData;

use actix::{Actor, AsyncContext, ContextFutureSpawner, StreamHandler, WrapFuture};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;

use crate::{
    codec::{ConnTypePayload, EraseDeserializer, Marshal, Unmarshal},
    error::Error,
    message::{ErrorMessage, ResponseHeader},
};

use super::{
    Arc, ArcAsyncServiceCall, AsyncServiceMap, HandlerResult, MessageId, RequestHeader, Server,
};

struct ServerActor<Codec: Unpin> {
    pub services: Arc<AsyncServiceMap>,
    pub req_header: Option<RequestHeader>,

    phantom: PhantomData<Codec>,
}

#[derive(actix::Message)]
#[rtype(result = "()")]
struct HandlerResultMessage {
    id: MessageId,
    res: HandlerResult,
}

impl<C> actix::Handler<HandlerResultMessage> for ServerActor<C>
where
    C: Marshal + Unmarshal + Unpin + 'static,
{
    type Result = ();

    fn handle(&mut self, msg: HandlerResultMessage, ctx: &mut Self::Context) -> Self::Result {
        let HandlerResultMessage { id, res } = msg;
        match Self::send_response_via_context(id, res, ctx) {
            Ok(_) => (),
            Err(e) => log::error!("Error encountered sending response via context: {}", e),
        };
    }
}

impl<C> Actor for ServerActor<C>
where
    C: Marshal + Unmarshal + Unpin + 'static,
{
    type Context = ws::WebsocketContext<Self>;
}

impl<C> StreamHandler<Result<ws::Message, ws::ProtocolError>> for ServerActor<C>
where
    C: Marshal + Unmarshal + EraseDeserializer + Unpin + 'static,
{
    fn handle(&mut self, item: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match item {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                log::error!(
                    "Received Text message: {} while expecting a binary message",
                    text
                );
            }
            Ok(ws::Message::Binary(bin)) => {
                match self.req_header.take() {
                    None => match C::unmarshal(&bin) {
                        Ok(h) => {
                            self.req_header.get_or_insert(h);
                        }
                        Err(e) => {
                            log::error!("Failed to unmarshal request header: {}", e);
                        }
                    },
                    Some(header) => {
                        // [0] read request body
                        let deserializer = C::from_bytes(bin.to_vec());
                        // [1] destructure header
                        let RequestHeader { id, service_method } = header;

                        // [2] split service name and method name
                        // return early send back Error::MethodNotFound if no "." is found
                        let pos = match service_method.rfind('.') {
                            Some(idx) => idx,
                            None => {
                                let err = Err(Error::MethodNotFound);
                                match Self::send_response_via_context(id, err, ctx) {
                                    Ok(_) => (),
                                    Err(e) => log::error!(
                                        "Error encountered sending response via context: {}",
                                        e
                                    ),
                                };
                                log::error!(
                                    "Method not supplied from request: '{}'",
                                    service_method
                                );
                                return;
                            }
                        };
                        let service = service_method[..pos].to_owned();
                        let method = service_method[pos + 1..].to_owned();
                        log::trace!(
                            "Message id: {}, service: {}, method: {}",
                            id,
                            service,
                            method
                        );

                        // [3] look up the service
                        // return early and send back Error::ServiceNotFound if key is not found
                        let call: ArcAsyncServiceCall = match self.services.get(&service[..]) {
                            Some(serv_call) => serv_call.clone(),
                            None => {
                                let err = Err(Error::ServiceNotFound);
                                match Self::send_response_via_context(id, err, ctx) {
                                    Ok(_) => (),
                                    Err(e) => log::error!(
                                        "Error encountered sending response via context: {}",
                                        e
                                    ),
                                };
                                log::error!("Service not found: '{}'", service);
                                return;
                            }
                        };

                        // [4] execute the call
                        let actor_addr = ctx.address().recipient();
                        let future = async move {
                            let res = call(method.clone(), deserializer).await.map_err(|err| {
                                log::error!(
                                    "Error found calling service: '{}', method: '{}', error: '{}'",
                                    service,
                                    method,
                                    err
                                );
                                match err {
                                    // if serde cannot parse request, the argument is likely mistaken
                                    Error::ParseError(e) => {
                                        log::error!("ParseError {:?}", e);
                                        Error::InvalidArgument
                                    }
                                    e @ _ => e,
                                }
                            });

                            match actor_addr.do_send(HandlerResultMessage { id, res }) {
                                Ok(_) => (),
                                Err(e) => {
                                    log::error!("Error encountered while sending message to actor. Error: {}", e);
                                }
                            };
                        };

                        future.into_actor(self).spawn(ctx);
                    }
                }
            }
            _ => (),
        }
    }
}

impl<C> ServerActor<C>
where
    C: Marshal + Unmarshal + Unpin + 'static,
{
    fn send_response_via_context(
        id: MessageId,
        res: HandlerResult,
        ctx: &mut <Self as Actor>::Context,
    ) -> Result<(), Error> {
        match res {
            Ok(body) => {
                log::trace!("Message {} Success", id.clone());
                let header = ResponseHeader {
                    id,
                    is_error: false,
                };
                let buf = C::marshal(&header)?;
                ctx.binary(buf);

                // serialize response body
                let buf = C::marshal(&body)?;
                ctx.binary(buf);
            }
            Err(err) => {
                log::trace!("Message {} Error", id.clone());
                let header = ResponseHeader { id, is_error: true };
                let msg = match ErrorMessage::from_err(err) {
                    Ok(m) => m,
                    Err(e) => {
                        log::error!("Cannot send back IoError or ParseError: {:?}", e);
                        return Err(e);
                    }
                };

                // compose error response header
                let buf = C::marshal(&header)?;
                ctx.binary(buf);

                // compose error response body
                // let body = match e {
                //     Error::RpcError(rpc_err) => Box::new(rpc_err),
                //     _ => Box::new(RpcError::ServerError(e.to_string())),
                // };
                let buf = C::marshal(&msg)?;
                ctx.binary(buf);
            }
        }

        Ok(())
    }
}

cfg_if! {
    if #[cfg(any(
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
        ),
        feature = "docs"
    ))] {
        use crate::codec::DefaultCodec;

        async fn index(
            state: web::Data<Server>,
            req: HttpRequest,
            stream: web::Payload,
        ) -> Result<HttpResponse, actix_web::Error> {
            let services = state.services.clone();
            let actor: ServerActor<DefaultCodec<Vec<u8>, Vec<u8>, ConnTypePayload>> = ServerActor {
                services,
                req_header: None,
                phantom: PhantomData,
            };
            let resp = ws::start(actor, &req, stream);
            resp
        }

        /// The following impl block is controlled by feature flag. It is enabled
        /// if and only if **exactly one** of the the following feature flag is turned on
        /// - `serde_bincode`
        /// - `serde_json`
        /// - `serde_cbor`
        /// - `serde_rmp`
        impl Server {
            #[cfg(any(feature = "http_actix_web", feature = "docs"))]
            #[cfg_attr(feature = "docs", doc(cfg(feature = "http_actix_web")))]
            /// Configuration for integration with an actix-web scope.
            /// A convenient funciont "handle_http" may be used to achieve the same thing
            /// with the `actix-web` feature turned on.
            ///
            /// The `DEFAULT_RPC_PATH` will be appended to the end of the scope's path.
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
            /// use toy_rpc::Server;
            /// use toy_rpc::macros::{export_impl, service};
            /// use actix_web::{App, HttpServer, web};
            ///
            /// struct FooService { }
            ///
            /// #[export_impl]
            /// impl FooService {
            ///     // define some "exported" functions
            /// }
            ///
            /// #[actix::main]
            /// async fn main() -> std::io::Result<()> {
            ///     let addr = "127.0.0.1:8080";
            ///
            ///     let foo_service = Arc::new(FooService { });
            ///
            ///     let server = Server::builder()
            ///         .register(foo_service)
            ///         .build();
            ///
            ///     let app_data = web::Data::new(server);
            ///
            ///     HttpServer::new(
            ///         move || {
            ///             App::new()
            ///                 .service(
            ///                     web::scope("/rpc/")
            ///                         .app_data(app_data.clone())
            ///                         .configure(Server::scope_config)
            ///                         // The line above may be replaced with line below
            ///                         //.configure(Server::handle_http()) // use the convenience `handle_http`
            ///                 )
            ///         }
            ///     )
            ///     .bind(addr)?
            ///     .run()
            ///     .await
            /// }
            /// ```
            ///
            pub fn scope_config(cfg: &mut web::ServiceConfig) {
                cfg.service(
                    web::scope("/")
                        .service(web::resource(super::DEFAULT_RPC_PATH).route(web::get().to(index))),
                );
            }

            #[cfg(any(all(feature = "http_actix_web", not(feature = "http_tide"),), feature = "docs"))]
            #[cfg_attr(
                feature = "docs",
                doc(cfg(all(feature = "http_actix_web", not(feature = "http_tide"))))
            )]
            /// A conevience function that calls the corresponding http handling
            /// function depending on the enabled feature flag
            ///
            /// | feature flag | function name  |
            /// | ------------ |---|
            /// | `http_tide`| [`into_endpoint`](#method.into_endpoint) |
            /// | `http_actix_web` | [`scope_config`](#method.scope_config) |
            /// | `http_warp` | [`into_boxed_filter`](#method.into_boxed_filter) |
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
            /// use toy_rpc::Server;
            /// use toy_rpc::macros::{export_impl, service};
            /// use actix_web::{App, web};
            ///
            /// struct FooService { }
            ///
            /// #[export_impl]
            /// impl FooService {
            ///     // define some "exported" functions
            /// }
            ///
            /// #[actix::main]
            /// async fn main() -> std::io::Result<()> {
            ///     let addr = "127.0.0.1:8080";
            ///
            ///     let foo_service = Arc::new(FooService { });
            ///
            ///     let server = Server::builder()
            ///         .register(foo_service)
            ///         .build();
            ///
            ///     let app_data = web::Data::new(server);
            ///
            ///     HttpServer::new(
            ///         move || {
            ///             App::new()
            ///                 .service(hello)
            ///                 .service(
            ///                     web::scope("/rpc/")
            ///                         .app_data(app_data.clone())
            ///                         .configure(Server::handle_http()) // use the convenience `handle_http`
            ///                 )
            ///         }
            ///     )
            ///     .bind(addr)?
            ///     .run()
            ///     .await
            /// }
            /// ```
            pub fn handle_http() -> fn(&mut web::ServiceConfig) {
                Self::scope_config
            }
        }
    }
}
