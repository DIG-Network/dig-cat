//! The unsigned-spend result types shared by every builder (issue / send / melt).

use chia_protocol::CoinSpend;
use chia_wallet_sdk::driver::Cat;

/// The unsigned output of a CAT builder (INV-3): the coin spends to sign + broadcast, the child
/// [`Cat`]s the spend creates (for chaining / lineage), and the value accounting.
///
/// The caller passes `coin_spends` to [`crate::required_signatures`], signs the reported messages,
/// assembles a `SpendBundle`, and broadcasts. dig-cat never signs (INV-2/INV-3).
#[derive(Debug, Clone)]
pub struct UnsignedCatSpend {
    /// The unsigned coin spends comprising the CAT operation.
    pub coin_spends: Vec<CoinSpend>,
    /// The child CATs the operation creates (recipients + change), each with its lineage proof.
    pub children: Vec<Cat>,
    /// The value plan (inputs / outputs / change / net delta) for this operation.
    pub plan: CatValuePlan,
}

/// The value accounting of a CAT operation, in base units.
///
/// `delta` is the net supply change the ring produces: `(outputs + change) - inputs`. It is `0` for a
/// conservative send (value is preserved) and negative for a melt (value is destroyed via the TAIL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatValuePlan {
    /// Total base units of the selected input coins.
    pub inputs: u64,
    /// Base units paid to recipients (excludes change back to the owner).
    pub outputs: u64,
    /// Base units returned to the owner as change.
    pub change: u64,
    /// Net supply change: `(outputs + change) - inputs`. `0` for send, `-(melt_amount)` for melt.
    pub delta: i128,
}

impl CatValuePlan {
    /// Build a plan from raw totals, computing `delta` as `(outputs + change) - inputs`.
    pub(crate) fn new(inputs: u64, outputs: u64, change: u64) -> Self {
        let delta = i128::from(outputs) + i128::from(change) - i128::from(inputs);
        Self {
            inputs,
            outputs,
            change,
            delta,
        }
    }
}
