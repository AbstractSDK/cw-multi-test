use crate::bank::NAMESPACE_BANK;
use crate::error::AnyResult;
use crate::{app::CosmosRouter, BankKeeper};
use cosmwasm_std::{Api, BlockInfo, Storage};

use crate::ibc::types::{AppIbcBasicResponse, AppIbcReceiveResponse};
use cosmwasm_std::{Addr, IbcPacketAckMsg, IbcPacketReceiveMsg};

use crate::error::bail;
use crate::prefixed_storage::prefixed;

use cosmwasm_std::{coins, from_json};
use cw20_ics20::ibc::Ics20Packet;

use super::IbcModule;
/// Address that locks the funds transfered through IBC
pub const IBC_LOCK_MODULE_ADDRESS: &str = "ibc_bank_lock_module";

pub fn wrap_ibc_denom(channel_id: String, denom: String) -> String {
    format!("ibc/{}/{}", channel_id, denom)
}

/// Helper to unwrap ibc denom
pub fn optional_unwrap_ibc_denom(denom: String, expected_channel_id: String) -> String {
    let split: Vec<_> = denom.splitn(3, '/').collect();
    if split.len() != 3 {
        return denom;
    }

    if split[0] != "ibc" {
        return denom;
    }

    if split[1] != expected_channel_id {
        return denom;
    }

    split[2].to_string()
}

impl IbcModule for BankKeeper {
    fn ibc_packet_receive<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        request: IbcPacketReceiveMsg,
    ) -> AnyResult<AppIbcReceiveResponse> {
        // When receiving a packet, one simply needs to unpack the amount and send that to the the receiver
        let packet: Ics20Packet = from_json(&request.packet.data)?;

        let mut bank_storage = prefixed(storage, NAMESPACE_BANK);

        // If the denom is exactly a denom that was sent through this channel, we can mint it directly without denom changes
        // This can be verified by checking the ibc_module mock balance
        let balances =
            self.get_balance(&bank_storage, &Addr::unchecked(IBC_LOCK_MODULE_ADDRESS))?;
        let locked_amount = balances.iter().find(|b| b.denom == packet.denom);

        if let Some(locked_amount) = locked_amount {
            assert!(
                locked_amount.amount >= packet.amount,
                "The ibc locked amount is lower than the packet amount"
            );
            // We send tokens from the IBC_LOCK_MODULE
            self.send(
                &mut bank_storage,
                Addr::unchecked(IBC_LOCK_MODULE_ADDRESS),
                api.addr_validate(&packet.receiver)?,
                coins(packet.amount.u128(), packet.denom),
            )?;
        } else {
            // Else, we receive the denom with prefixes
            self.mint(
                &mut bank_storage,
                api.addr_validate(&packet.receiver)?,
                coins(
                    packet.amount.u128(),
                    wrap_ibc_denom(request.packet.dest.channel_id, packet.denom),
                ),
            )?;
        }

        // No acknowledgment needed
        Ok(AppIbcReceiveResponse::default())
    }

    fn ibc_packet_acknowledge<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _request: IbcPacketAckMsg,
    ) -> AnyResult<AppIbcBasicResponse> {
        // Acknowledgment can't fail, so no need for ack response parsing
        Ok(AppIbcBasicResponse::default())
    }

    fn ibc_packet_timeout<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        request: cosmwasm_std::IbcPacketTimeoutMsg,
    ) -> AnyResult<AppIbcBasicResponse> {
        // On timeout, we unpack the amount and sent that back to the receiverwe give the funds back to the sender of the packet

        // When receiving a packet, one simply needs to unpack the amount and send that to the the receiver
        let packet: Ics20Packet = from_json(request.packet.data)?;

        let mut bank_storage = prefixed(storage, NAMESPACE_BANK);

        // We verify the denom is exactly a denom that was sent through this channel
        // This can be verified by checking the ibc_module mock balance
        let balances =
            self.get_balance(&bank_storage, &Addr::unchecked(IBC_LOCK_MODULE_ADDRESS))?;
        let locked_amount = balances.iter().find(|b| b.denom == packet.denom);

        if let Some(locked_amount) = locked_amount {
            assert!(
                locked_amount.amount >= packet.amount,
                "The ibc locked amount is lower than the packet amount"
            );
            // We send tokens from the IBC_LOCK_MODULE
            self.send(
                &mut bank_storage,
                Addr::unchecked(IBC_LOCK_MODULE_ADDRESS),
                api.addr_validate(&packet.sender)?,
                coins(packet.amount.u128(), packet.denom),
            )?;
        } else {
            bail!("Funds refund after a timeout, can't timeout a transfer that was not initiated")
        }

        Ok(AppIbcBasicResponse::default())
    }
}
