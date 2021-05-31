# Quickstart

A simple quickstart with `tokio` runtime is shown below. More examples can be found in the **Example** chapter.

## Initialize new project

`cargo new --lib toy_rpc_quickstart`

## Add dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", ] }
toy-rpc = { version = "0.7.0-alpha.0, feature = ["tokio_runtime", "server", "client"] }
```

## Project structure

```
./src
├── /bin
│   ├── server.rs
│   ├── client.rs
└── lib.rs
```

In the `Cargo.toml`, you may need to specify the binaries with 

```toml
[[bin]]
name = "server"
path = "src/bin/server.rs"

[[bin]]
name = "client"
path = "src/bin/client.rs" 
```

## Define RPC service

In `src/lib.rs`

```rust 
// src/lib.rs

mod rpc {
    use toy_rpc::macros::export_impl;
    pub struct Echo { }
    
    #[export_impl]
    impl Echo {
        #[export_method]
        pub async fn echo_i32(&self, arg: i32) -> Result<i32, String> {
            Ok(arg)
        }
    }
}
```

## RPC server

In `src/bin/server.rs`

```rust 
// src/bin/server.rs

use tokio::{task, net::TcpListener};
use std::sync::Arc;
use toy_rpc::Server;

use toy_rpc_quickstart::rpc::Echo;

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:23333";
    
    // Creates an instance of the `Echo` service
    let echo_service = Arc::new(
        Echo { }
    );

    let server = Server::builder()
        .register(echo_service) // register service
        .build();
    let listener = TcpListener::bind(addr).await.unwrap();

    // Run the server in a separate task
    let handle = task::spawn(async move {
        println!("Starting server at {}", &addr);
        server.accept(listener).await.unwrap();
    });
    handle.await.expect("Error running the RPC server");
}
```

## RPC client

In `src/bin/client.rs`

```rust 

```