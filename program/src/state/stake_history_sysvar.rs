//! History of stake activations and de-activations.
//!
//! The _stake history sysvar_ provides access to the [`StakeHistory`] type.
//!
//! The [`Sysvar::get`] method always returns
//! [`ProgramError::UnsupportedSysvar`], and in practice the data size of this
//! sysvar is too large to process on chain. One can still use the
//! [`SysvarId::id`], [`SysvarId::check_id`] and [`Sysvar::size_of`] methods in
//! an on-chain program, and it can be accessed off-chain through RPC.
//!
//! [`ProgramError::UnsupportedSysvar`]: https://docs.rs/solana-program-error/latest/solana_program_error/enum.ProgramError.html#variant.UnsupportedSysvar
//! [`SysvarId::id`]: https://docs.rs/solana-sysvar-id/latest/solana_sysvar_id/trait.SysvarId.html
//! [`SysvarId::check_id`]: https://docs.rs/solana-sysvar-id/latest/solana_sysvar_id/trait.SysvarId.html#tymethod.check_id

use pinocchio::sysvars::clock::Epoch;

pub mod stake_history_id {
    pinocchio_pubkey::declare_id!("SysvarS1otHistory11111111111111111111111111");
}

pub use stake_history_id::{check_id, id, ID};
pub const MAX_ENTRIES: usize = 512; // it should never take as many as 512 epochs to warm up or cool down

use crate::state::get_sysvar;

use super::{StakeHistoryEntry, StakeHistoryGetEntry};

// we do not provide Default because this requires the real current epoch
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct StakeHistorySysvar(pub Epoch);

// precompute so we can statically allocate buffer
const EPOCH_AND_ENTRY_SERIALIZED_SIZE: u64 = 32;

impl StakeHistoryGetEntry for StakeHistorySysvar {
    fn get_entry(&self, target_epoch: Epoch) -> Option<StakeHistoryEntry> {
        let current_epoch = self.0;

        // if current epoch is zero this returns None because there is no history yet
        let newest_historical_epoch = current_epoch.checked_sub(1)?;
        let oldest_historical_epoch = current_epoch.saturating_sub(MAX_ENTRIES as u64);

        // target epoch is old enough to have fallen off history; presume fully active/deactive
        if target_epoch < oldest_historical_epoch {
            return None;
        }

        // epoch delta is how many epoch-entries we offset in the stake history vector, which may be zero
        // None means target epoch is current or in the future; this is a user error
        let epoch_delta = newest_historical_epoch.checked_sub(target_epoch)?;

        // offset is the number of bytes to our desired entry, including eight for vector length
        let offset = epoch_delta
            .checked_mul(EPOCH_AND_ENTRY_SERIALIZED_SIZE)?
            .checked_add(core::mem::size_of::<u64>() as u64)?;

        let mut entry_buf = [0; EPOCH_AND_ENTRY_SERIALIZED_SIZE as usize];
        let result = get_sysvar(
            &mut entry_buf,
            &id(),
            offset,
            EPOCH_AND_ENTRY_SERIALIZED_SIZE,
        );

        match result {
            Ok(()) => {
                // All safe because `entry_buf` is a 32-length array
                let entry_epoch: [u8; 8] = entry_buf[0..8].try_into().unwrap();
                let effective = entry_buf[8..16].try_into().unwrap();
                let activating = entry_buf[16..24].try_into().unwrap();
                let deactivating = entry_buf[24..32].try_into().unwrap();

                // this would only fail if stake history skipped an epoch or the binary format of the sysvar changed
                assert_eq!(u64::from_le_bytes(entry_epoch), target_epoch);

                Some(StakeHistoryEntry {
                    effective,
                    activating,
                    deactivating,
                })
            }
            _ => None,
        }
    }
}

/*

//---------------------------- Fix Tests Later ----------------------------------------
#[cfg(test)]
mod tests {
    use crate::state::StakeHistory;

    use super::*;

    #[test]
    fn test_stake_history() {
        let mut stake_history = StakeHistory::default();

        for i in 0..MAX_ENTRIES as u64 + 1 {
            stake_history.add(
                i,
                StakeHistoryEntry {
                    activating: i,
                    ..StakeHistoryEntry::default()
                },
            );
        }
        assert_eq!(stake_history.len(), MAX_ENTRIES);
        assert_eq!(stake_history.iter().map(|entry| entry.0).min().unwrap(), 1);
        assert_eq!(stake_history.get(0), None);
        assert_eq!(
            stake_history.get(1),
            Some(&StakeHistoryEntry {
                activating: 1,
                ..StakeHistoryEntry::default()
            })
        );
    }

    #[test]
    fn test_id() {
        assert_eq!(StakeHistory::id(), crate::helpers::stake_history::id());
    }

    #[test]
    fn test_size_of() {
        let mut stake_history = StakeHistory::default();
        for i in 0..MAX_ENTRIES as u64 {
            stake_history.add(
                i,
                StakeHistoryEntry {
                    activating: i,
                    ..StakeHistoryEntry::default()
                },
            );
        }

        assert_eq!(
            bincode::serialized_size(&stake_history).unwrap() as usize,

            StakeHistory::size_of()
        );

        let stake_history_inner: Vec<(Epoch, StakeHistoryEntry)> =
            bincode::deserialize(&bincode::serialize(&stake_history).unwrap()).unwrap();
        let epoch_entry = stake_history_inner.into_iter().next().unwrap();

        assert_eq!(
            bincode::serialized_size(&epoch_entry).unwrap(),
            EPOCH_AND_ENTRY_SERIALIZED_SIZE
        );
    }

    // TODO
    //#[serial]
    #[test]
    fn test_stake_history_get_entry() {
        let unique_entry_for_epoch = |epoch: u64| StakeHistoryEntry {
            activating: epoch.saturating_mul(2),
            deactivating: epoch.saturating_mul(3),
            effective: epoch.saturating_mul(5),
        };

        let current_epoch = MAX_ENTRIES.saturating_add(2) as u64;

        // make a stake history object with at least one valid entry that has expired
        let mut stake_history = StakeHistory::default();
        for i in 0..current_epoch {
            stake_history.add(i, unique_entry_for_epoch(i));
        }
        assert_eq!(stake_history.len(), MAX_ENTRIES);
        assert_eq!(stake_history.iter().map(|entry| entry.0).min().unwrap(), 2);

        // set up sol_get_sysvar

        // TODO

        //mock_get_sysvar_syscall(&bincode::serialize(&stake_history).unwrap());

        // make a syscall interface object
        let stake_history_sysvar = StakeHistorySysvar(current_epoch);

        // now test the stake history interfaces

        assert_eq!(stake_history.get(0), None);
        assert_eq!(stake_history.get(1), None);
        assert_eq!(stake_history.get(current_epoch), None);

        assert_eq!(stake_history.get_entry(0), None);
        assert_eq!(stake_history.get_entry(1), None);
        assert_eq!(stake_history.get_entry(current_epoch), None);

        assert_eq!(stake_history_sysvar.get_entry(0), None);
        assert_eq!(stake_history_sysvar.get_entry(1), None);
        assert_eq!(stake_history_sysvar.get_entry(current_epoch), None);

        for i in 2..current_epoch {
            let entry = Some(unique_entry_for_epoch(i));

            assert_eq!(stake_history.get(i), entry.as_ref(),);

            assert_eq!(stake_history.get_entry(i), entry,);

            assert_eq!(stake_history_sysvar.get_entry(i), entry,);
        }
    }

    // TODO
    //#[serial]
    #[test]
    fn test_stake_history_get_entry_zero() {
        let mut current_epoch = 0;

        // first test that an empty history returns None
        let stake_history = StakeHistory::default();
        assert_eq!(stake_history.len(), 0);

        //mock_get_sysvar_syscall(&bincode::serialize(&stake_history).unwrap());
        let stake_history_sysvar = StakeHistorySysvar(current_epoch);

        assert_eq!(stake_history.get(0), None);
        assert_eq!(stake_history.get_entry(0), None);
        assert_eq!(stake_history_sysvar.get_entry(0), None);

        // next test that we can get a zeroth entry in the first epoch
        let entry_zero = StakeHistoryEntry {
            effective: 100,
            ..StakeHistoryEntry::default()
        };
        let entry = Some(entry_zero.clone());

        let mut stake_history = StakeHistory::default();
        stake_history.add(current_epoch, entry_zero);
        assert_eq!(stake_history.len(), 1);
        current_epoch = current_epoch.saturating_add(1);

        // TODO
        // mock_get_sysvar_syscall(&bincode::serialize(&stake_history).unwrap());
        let stake_history_sysvar = StakeHistorySysvar(current_epoch);

        assert_eq!(stake_history.get(0), entry.as_ref());
        assert_eq!(stake_history.get_entry(0), entry);
        assert_eq!(stake_history_sysvar.get_entry(0), entry);

        // finally test that we can still get a zeroth entry in later epochs
        stake_history.add(current_epoch, StakeHistoryEntry::default());
        assert_eq!(stake_history.len(), 2);
        current_epoch = current_epoch.saturating_add(1);

        // TODO
        // mock_get_sysvar_syscall(&bincode::serialize(&stake_history).unwrap());
        let stake_history_sysvar = StakeHistorySysvar(current_epoch);

        assert_eq!(stake_history.get(0), entry.as_ref());
        assert_eq!(stake_history.get_entry(0), entry);
        assert_eq!(stake_history_sysvar.get_entry(0), entry);
    }
}
 */
