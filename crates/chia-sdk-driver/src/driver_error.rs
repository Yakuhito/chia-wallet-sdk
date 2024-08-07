use chia_sdk_types::conditions::ConditionError;
use clvm_traits::{FromClvmError, ToClvmError};
use clvmr::reduction::EvalErr;
use thiserror::Error;

use crate::SpendError;

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("failed to serialize clvm value: {0}")]
    ToClvm(#[from] ToClvmError),

    #[error("failed to deserialize clvm value: {0}")]
    FromClvm(#[from] FromClvmError),

    #[error("failed to parse conditions: {0}")]
    Conditions(#[from] ConditionError),

    #[error("spend error: {0}")]
    Spend(#[from] SpendError),

    #[error("clvm eval error: {0}")]
    Eval(#[from] EvalErr),

    #[error("custom driver error: {0}")]
    Custom(String),

    #[error("invalid mod hash")]
    InvalidModHash,

    #[error("non-standard inner puzzle layer")]
    NonStandardLayer,

    #[error("missing child")]
    MissingChild,

    #[error("missing CREATE_COIN condition in parent spend")]
    MissingParentCreateCoin,

    #[error("missing hint")]
    MissingHint,

    #[error("invalid singleton struct")]
    InvalidSingletonStruct,

    #[error("mismatched singleton output (maybe no spend revealed the new singleton state)")]
    MismatchedOutput,

    #[error(
        "missing puzzle (required to build innermost puzzle - usually fixed by using .with_puzzle)"
    )]
    MissingPuzzle,
}
