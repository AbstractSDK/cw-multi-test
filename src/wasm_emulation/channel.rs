use crate::wasm_emulation::input::SerChainData;
use anyhow::Result as AnyResult;
use cw_orch_daemon::GrpcChannel;
use tokio::runtime::Runtime;
use tonic::transport::Channel;

pub fn get_rt_and_channel(chain: impl Into<SerChainData>) -> AnyResult<(Runtime, Channel)> {
    let rt = Runtime::new()?;
    let chain = chain.into();
    // We create an instance from a code_id, an address, and we run the code in it
    let channel = rt.block_on(GrpcChannel::connect(&chain.apis.grpc, &chain.chain_id))?;

    Ok((rt, channel))
}

pub fn get_channel(chain: impl Into<SerChainData>, rt: &Runtime) -> AnyResult<Channel> {
    let chain = chain.into();
    // We create an instance from a code_id, an address, and we run the code in it
    let channel = rt.block_on(GrpcChannel::connect(&chain.apis.grpc, &chain.chain_id))?;

    Ok(channel)
}
