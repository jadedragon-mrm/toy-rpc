use serde::{Serialize, Deserialize};

use toy_rpc::pubsub::Topic;

pub const ADDR: &str = "127.0.0.1:23333";

#[derive(Debug, Serialize, Deserialize)]
pub struct Count(pub u32);

impl Topic for Count {
    type Item = Count;

    fn topic() -> String {
        "Count".into()
    }
}