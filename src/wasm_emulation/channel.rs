use anyhow::Result as AnyResult;
use cw_orch::daemon::GrpcChannel;
use cw_orch::prelude::ChainInfoOwned;
use tokio::runtime::{Handle, Runtime};
use tonic::transport::Channel;

#[derive(Clone)]
pub struct RemoteChannel {
    pub rt: Handle,
    pub channel: Channel,
    pub pub_address_prefix: String,
}

impl RemoteChannel {
    pub fn new(
        rt: &Runtime,
        channel: Channel,
        pub_address_prefix: impl Into<String>,
    ) -> AnyResult<Self> {
        Ok(Self {
            rt: rt.handle().clone(),
            channel,
            pub_address_prefix: pub_address_prefix.into(),
        })
    }
}
