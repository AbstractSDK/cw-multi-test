use anyhow::Result as AnyResult;
use cw_orch::{daemon::GrpcChannel, environment::ChainInfoOwned};
use tokio::runtime::{Handle, Runtime};
use tonic::transport::Channel;

/// Simple helper to get the GRPC transport channel
fn get_channel(
    chain: impl Into<ChainInfoOwned>,
    rt: &Runtime,
) -> anyhow::Result<tonic::transport::Channel> {
    let chain = chain.into();
    let channel = rt.block_on(GrpcChannel::connect(&chain.grpc_urls, &chain.chain_id))?;
    Ok(channel)
}

#[derive(Clone)]
pub struct RemoteChannel {
    pub rt: Handle,
    pub channel: Channel,
    pub pub_address_prefix: String,
}

impl RemoteChannel {
    pub fn new(
        rt: &Runtime,
        chain: impl Into<ChainInfoOwned>,
        pub_address_prefix: impl Into<String>,
    ) -> AnyResult<Self> {
        Ok(Self {
            rt: rt.handle().clone(),
            channel: get_channel(chain, rt)?,
            pub_address_prefix: pub_address_prefix.into(),
        })
    }
}
