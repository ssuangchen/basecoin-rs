use std::fmt::Debug;

use basecoin_app::BaseCoinApp;
use basecoin_modules::auth::{AuthAccountKeeper, AuthAccountReader};
use basecoin_modules::bank::Bank;
use basecoin_modules::context::{prefix, Identifiable};
use basecoin_modules::ibc::Ibc;
use basecoin_store::context::ProvableStore;
use basecoin_store::utils::SharedRwExt;

/// Gives access to the IBC module.
pub fn ibc<S>(app: BaseCoinApp<S>) -> Ibc<S>
where
    S: ProvableStore + Default + Debug,
{
    let modules = app.modules.read_access();

    modules
        .iter()
        .find(|m| m.id == prefix::Ibc {}.identifier())
        .and_then(|m| m.module.as_any().downcast_ref::<Ibc<S>>().cloned())
        .expect("IBC module not found")
}

/// Gives access to the Bank module.
pub fn bank<S>(app: BaseCoinApp<S>) -> Bank<S, AuthAccountReader<S>, AuthAccountKeeper<S>>
where
    S: ProvableStore + Default + Debug,
{
    let modules = app.modules.read_access();

    modules
        .iter()
        .find(|m| m.id == prefix::Bank {}.identifier())
        .and_then(|m| {
            m.module
                .as_any()
                .downcast_ref::<Bank<S, AuthAccountReader<S>, AuthAccountKeeper<S>>>()
                .cloned()
        })
        .expect("Bank module not found")
}
