use chia_bls::PublicKey;
use chia_protocol::Bytes32;
use chia_puzzles::{standard::StandardArgs, EveProof, Proof};
use clvm_traits::{FromClvm, ToClvm, ToNodePtr};
use clvm_utils::{tree_hash_atom, CurriedProgram, ToTreeHash};
use clvmr::NodePtr;

use crate::{Conditions, DriverError, Launcher, SpendContext, SpendError};

use super::Did;

impl Launcher {
    pub fn create_eve_did<M>(
        self,
        ctx: &mut SpendContext,
        p2_puzzle_hash: Bytes32,
        p2_puzzle: Option<NodePtr>,
        recovery_did_list_hash: Bytes32,
        num_verifications_required: u64,
        metadata: M,
    ) -> Result<(Conditions, Did<M>, Proof), SpendError>
    where
        M: ToClvm<NodePtr> + FromClvm<NodePtr> + Clone + ToTreeHash,
    {
        let launcher_coin = self.coin();
        let did = Did::new(
            self.coin(), // fake coin to get inner ph
            launcher_coin.coin_id(),
            recovery_did_list_hash,
            num_verifications_required,
            metadata,
            p2_puzzle_hash.into(),
            p2_puzzle,
        );

        let inner_puzzle_hash = did.singleton_inner_puzzle_hash().into();

        let (launch_singleton, eve_coin) = self.spend(ctx, inner_puzzle_hash, ())?;

        let proof = Proof::Eve(EveProof {
            parent_coin_info: launcher_coin.parent_coin_info,
            amount: launcher_coin.amount,
        });

        Ok((launch_singleton, did.with_coin(eve_coin), proof))
    }

    pub fn create_did<M>(
        self,
        ctx: &mut SpendContext,
        recovery_did_list_hash: Bytes32,
        num_verifications_required: u64,
        metadata: M,
        synthetic_key: PublicKey,
    ) -> Result<(Conditions, Did<M>, Proof), DriverError>
    where
        M: ToClvm<NodePtr> + FromClvm<NodePtr> + Clone + ToTreeHash,
        Self: Sized,
    {
        let inner_puzzle = CurriedProgram {
            program: ctx.standard_puzzle()?,
            args: StandardArgs { synthetic_key },
        }
        .to_node_ptr(ctx.allocator_mut())?;
        let inner_puzzle_hash = StandardArgs::curry_tree_hash(synthetic_key).into();

        let (create_did, did, eve_proof) = self.create_eve_did(
            ctx,
            inner_puzzle_hash,
            Some(inner_puzzle),
            recovery_did_list_hash,
            num_verifications_required,
            metadata,
        )?;

        let (new_did, new_proof) =
            ctx.spend_standard_did(&did, eve_proof, synthetic_key, Conditions::new())?;

        Ok((create_did, new_did, new_proof))
    }

    pub fn create_simple_did(
        self,
        ctx: &mut SpendContext,
        synthetic_key: PublicKey,
    ) -> Result<(Conditions, Did<()>, Proof), DriverError>
    where
        Self: Sized,
    {
        self.create_did(ctx, tree_hash_atom(&[]).into(), 1, (), synthetic_key)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Launcher, SpendContext};

    use chia_puzzles::standard::StandardArgs;
    use chia_sdk_test::{secret_key, test_transaction, Simulator};

    #[tokio::test]
    async fn test_create_did() -> anyhow::Result<()> {
        let sim = Simulator::new().await?;
        let peer = sim.connect().await?;
        let ctx = &mut SpendContext::new();

        let sk = secret_key()?;
        let pk = sk.public_key();

        let puzzle_hash = StandardArgs::curry_tree_hash(pk).into();
        let coin = sim.mint_coin(puzzle_hash, 1).await;

        let (launch_singleton, did, _) =
            Launcher::new(coin.coin_id(), 1).create_simple_did(ctx, pk)?;

        ctx.spend_p2_coin(coin, pk, launch_singleton)?;

        test_transaction(
            &peer,
            ctx.take_spends(),
            &[sk],
            sim.config().genesis_challenge,
        )
        .await;

        // Make sure the DID was created.
        let coin_state = sim
            .coin_state(did.coin.coin_id())
            .await
            .expect("expected did coin");
        assert_eq!(coin_state.coin, did.coin);

        Ok(())
    }
}
