

use toy_rpc::error::Error;
use toy_rpc::Client;

use tokio_tcp::rpc::{BarRequest, BarResponse, FooRequest, FooResponse};

#[tokio::main]
async fn main() {
    env_logger::init();

    let addr = "127.0.0.1:23333";
    let client = Client::dial(addr).await.unwrap();

    // first request, echo
    let args = FooRequest { a: 1, b: 3 };
    let reply: Result<FooResponse, Error> = client.call("foo_service.echo", &args);
    println!("{:?}", reply);

    // second request, increment_a
    // let args = FooRequest {
    //     a: reply.a,
    //     b: reply.b,
    // };
    let reply: Result<FooResponse, Error> = client.async_call("foo_service.increment_a", &args).await;
    println!("{:?}", reply);

    // second request, increment_b
    // let args = FooRequest {
    //     a: reply.a,
    //     b: reply.b,
    // };
    let handle = client.spawn_task("foo_service.increment_b", args);

    // Please note that tokio's JoinHandle is slightly 
    // different from that of async-std's
    let reply: Result<FooResponse, Error> = handle.await.unwrap();
    println!("{:?}", reply);

    // third request, bar echo
    let args = BarRequest {
        content: "bar".to_string(),
    };
    let reply: BarResponse = client.call("bar_service.echo", &args).unwrap();
    println!("{:?}", reply);

    // fourth request, bar exclaim
    let reply: BarResponse = client.async_call("bar_service.exclaim", &args).await.unwrap();
    println!("{:?}", reply);

    // third request, get_counter
    let args = ();
    let handle = client.spawn_task("foo_service.get_counter", args);

    // Please note that tokio's JoinHandle is slightly 
    // different from that of async-std's
    let reply: u32 = handle.await.unwrap().unwrap();
    println!("{:?}", reply);
}