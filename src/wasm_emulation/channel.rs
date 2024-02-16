use anyhow::Result as AnyResult;
use cw_orch_daemon::GrpcChannel;
use ibc_chain_registry::chain::ChainData;
use tokio::runtime::{Handle, Runtime};
use tonic::transport::Channel;

fn get_channel(chain: impl Into<ChainData>, rt: Handle) -> AnyResult<Channel> {
    let chain = chain.into();
    let channel = rt.block_on(GrpcChannel::connect(&chain.apis.grpc, &chain.chain_id))?;
    Ok(channel)
}

#[derive(Clone)]
pub struct RemoteChannel {
    pub rt: Handle,
    pub channel: Channel,
    pub chain: ChainData,
}

impl RemoteChannel {
    pub fn new(rt: &Runtime, chain: impl Into<ChainData>) -> AnyResult<Self> {
        let chain = chain.into();
        let channel = get_channel(chain.clone(), rt.handle().clone())?;
        Ok(Self {
            rt: rt.handle().clone(),
            channel,
            chain,
        })
    }
}
