use async_trait::async_trait;
use toy_rpc::macros::{export_trait, export_trait_impl};

fn anyhow_fun() -> anyhow::Result<()> {
    anyhow::bail!("error");
}

#[async_trait]
#[export_trait(impl_for_client)]
pub trait Arith {
    #[export_method]
    async fn foo(&self, _args: ()) -> Result<(), toy_rpc::Error>;
}

struct TestImpl {}

#[async_trait]
#[export_trait_impl]
impl Arith for TestImpl {
    async fn foo(&self, _args: ()) -> Result<(), toy_rpc::Error> {
        anyhow_fun()?; // the trait `From<anyhow::Error>` is not implemented for `toy_rpc::Error`
        Ok(())
    }
}