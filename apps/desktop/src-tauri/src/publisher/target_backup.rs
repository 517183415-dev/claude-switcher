//! Conservative merge of target-specific publication evidence during restore.
use super::{
    Error, HISTORY_LIMIT, Publisher, Result, STATE_LIMIT, State, TARGET_LIMIT, TargetLedger, encode,
};
use std::collections::BTreeMap;

fn ledgers(state: &State) -> Result<BTreeMap<String, TargetLedger>> {
    let mut result = state.other_targets.clone();
    if let Some(target) = &state.target {
        if result
            .insert(target.clone(), state.active_ledger())
            .is_some()
        {
            return Err(Error::Invalid("publisher_target"));
        }
    } else if state.has_publication_evidence() && !state.draft_only_recovery {
        return Err(Error::Invalid("legacy_target_unknown"));
    }
    Ok(result)
}

fn merge_ledger(local: &mut TargetLedger, restored: TargetLedger) -> Result<()> {
    for index in 0..2 {
        let left = &local.sequences[index];
        let right = &restored.sequences[index];
        if left.highest != 0
            && left.highest == right.highest
            && left.payload_hash != right.payload_hash
        {
            return Err(Error::Invalid("backup_sequence_conflict"));
        }
        if right.highest > left.highest {
            local.sequences[index] = right.clone();
        }
    }
    for record in restored.history {
        if let Some(existing) = local.history.iter().find(|existing| {
            existing.manifest.release_id == record.manifest.release_id
                && existing.manifest.channel == record.manifest.channel
        }) {
            if existing.manifest != record.manifest || existing.payload_hash != record.payload_hash
            {
                return Err(Error::Invalid("backup_history_conflict"));
            }
        } else {
            local.history.push(record);
        }
    }
    if local.history.len() > HISTORY_LIMIT {
        return Err(Error::Invalid("backup_history_capacity"));
    }
    if let Some(pending) = restored.pending {
        if let Some(existing) = &local.pending {
            if encode(existing, STATE_LIMIT)? != encode(&pending, STATE_LIMIT)? {
                return Err(Error::PendingOperation);
            }
        } else {
            local.pending = Some(pending);
        }
    }
    if let Some(resolution) = restored.last_resolution {
        if let Some(existing) = &local.last_resolution {
            if encode(existing, STATE_LIMIT)? != encode(&resolution, STATE_LIMIT)? {
                return Err(Error::ResolutionOccupied);
            }
        } else {
            local.last_resolution = Some(resolution);
        }
    }
    local.recovery_required = true;
    Ok(())
}

pub(super) fn merge(publisher: &Publisher, restored: &mut State) -> Result<()> {
    let mut targets = ledgers(&publisher.state)?;
    for (identity, ledger) in ledgers(restored)? {
        if let Some(local) = targets.get_mut(&identity) {
            merge_ledger(local, ledger)?;
        } else {
            let mut ledger = ledger;
            ledger.recovery_required = true;
            targets.insert(identity, ledger);
        }
    }
    if targets.len() > TARGET_LIMIT {
        return Err(Error::Invalid("publisher_target_capacity"));
    }
    for ledger in targets.values_mut() {
        ledger.recovery_required = true;
    }
    let active = publisher
        .state
        .target
        .clone()
        .or_else(|| restored.target.clone())
        .ok_or(Error::Invalid("publisher_target_unavailable"))?;
    let active_ledger = targets
        .remove(&active)
        .ok_or(Error::Invalid("publisher_target_unavailable"))?;
    restored.apply_ledger(active_ledger);
    restored.schema = 3;
    restored.target = Some(active);
    restored.other_targets = targets;
    restored.draft_only_recovery = false;
    Ok(())
}
