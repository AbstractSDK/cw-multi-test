use anyhow::Result as AnyResult;
use cw_orch::daemon::GrpcChannel;
use cw_orch::prelude::ChainInfoOwned;
use tokio::runtime::{Handle, Runtime};
use tonic::transport::Channel;

fn get_channel(chain: impl Into<ChainInfoOwned>, rt: Handle) -> AnyResult<Channel> {
    let chain = chain.into();
    let channel = rt.block_on(GrpcChannel::connect(&chain.grpc_urls, &chain.chain_id))?;
    Ok(channel)
}

#[derive(Clone)]
pub struct RemoteChannel {
    pub rt: Handle,
    pub channel: Channel,
    pub chain: ChainInfoOwned,
}

impl RemoteChannel {
    pub fn new(rt: &Runtime, chain: impl Into<ChainInfoOwned>) -> AnyResult<Self> {
        let chain = chain.into();
        let channel = get_channel(chain.clone(), rt.handle().clone())?;
        Ok(Self {
            rt: rt.handle().clone(),
            channel,
            chain,
        })
    }
}
