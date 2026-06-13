#![allow(clippy::needless_range_loop)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    pub id: i64,
    pub name: String,
    pub default_duration: i32, // in minutes
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduleBlock {
    pub id: Option<i64>,
    pub task_id: Option<i64>, // None represents "No plan"
    pub task_name: String,    // Display name: e.g., "No plan" if task_id is None
    pub day: String,          // YYYY-MM-DD
    pub start_minute: i32,    // 0 to 1439
    pub duration_minutes: i32,
    pub is_done: bool,
}

/// Recalculates start_minute for all blocks sequentially to ensure perfect contiguity.
pub fn recalculate_start_times(blocks: &mut [ScheduleBlock]) {
    let mut current_minute = 0;
    for block in blocks.iter_mut() {
        block.start_minute = current_minute;
        current_minute += block.duration_minutes;
    }
}

/// Consolidates contiguous "No plan" blocks to prevent fragmentation.
pub fn consolidate_blocks(blocks: &mut Vec<ScheduleBlock>) {
    if blocks.is_empty() {
        return;
    }

    let mut i = 1;
    while i < blocks.len() {
        if blocks[i - 1].task_id.is_none() && blocks[i].task_id.is_none() {
            // Both are "No plan", merge duration
            blocks[i - 1].duration_minutes += blocks[i].duration_minutes;
            blocks.remove(i);
        } else {
            i += 1;
        }
    }
    recalculate_start_times(blocks);
}

/// Adjusts the block at `index` by `delta_minutes`. Shims the difference from "No plan" blocks
/// downstream (or the last block if no "No plan" exists) to preserve the 24h day sum.
/// Returns true if the adjustment succeeded.
pub fn adjust_block_duration(blocks: &mut Vec<ScheduleBlock>, index: usize, delta_minutes: i32) -> bool {
    if index >= blocks.len() || delta_minutes == 0 {
        return false;
    }

    let current_duration = blocks[index].duration_minutes;
    if current_duration + delta_minutes <= 0 {
        return false; // Cannot shrink a task to 0 or negative duration
    }

    // Identify target blocks downstream to absorb the opposite delta
    // If delta > 0, we need to shrink downstream blocks
    // If delta < 0, we need to expand downstream blocks
    if delta_minutes > 0 {
        // We need to find "No plan" blocks after `index` to shrink
        let mut target_indices = Vec::new();
        for i in (index + 1)..blocks.len() {
            if blocks[i].task_id.is_none() {
                target_indices.push(i);
            }
        }

        // If no "No plan" block exists downstream, fall back to the very last block
        if target_indices.is_empty() && index + 1 < blocks.len() {
            target_indices.push(blocks.len() - 1);
        }

        if target_indices.is_empty() {
            return false; // Nowhere to absorb expansion
        }

        // Calculate if we have enough duration in target blocks to absorb the expansion
        let total_available: i32 = target_indices.iter().map(|&i| blocks[i].duration_minutes).sum();
        if total_available < delta_minutes {
            return false; // Not enough buffer to expand
        }

        // Perform absorption
        let mut remaining_delta = delta_minutes;
        for &t_idx in &target_indices {
            let avail = blocks[t_idx].duration_minutes;
            if avail >= remaining_delta {
                blocks[t_idx].duration_minutes -= remaining_delta;
                break;
            } else {
                blocks[t_idx].duration_minutes = 0;
                remaining_delta -= avail;
            }
        }

        blocks[index].duration_minutes += delta_minutes;
    } else {
        // delta_minutes < 0: we are contracting this block. We must expand a downstream block.
        // Prefer expanding the first downstream "No plan" block, or the last block.
        let mut target_idx = None;
        for i in (index + 1)..blocks.len() {
            if blocks[i].task_id.is_none() {
                target_idx = Some(i);
                break;
            }
        }

        let t_idx = target_idx.unwrap_or_else(|| {
            if index + 1 < blocks.len() {
                blocks.len() - 1
            } else {
                index // Fallback to itself if it's the last block, which doesn't make sense but prevents panic
            }
        });

        if t_idx == index {
            return false; // Cannot adjust the last block alone
        }

        blocks[index].duration_minutes += delta_minutes; // contract (since delta < 0)
        blocks[t_idx].duration_minutes -= delta_minutes; // expand (subtract negative is addition)
    }

    // Cleanup 0 duration blocks
    blocks.retain(|b| b.duration_minutes > 0 || b.task_id.is_none()); // keep "No plan" even if 0 temporarily, consolidate will clean
    consolidate_blocks(blocks);
    true
}

/// Splits the block at `index` (usually "No plan") to insert a scheduled task of `duration_minutes`.
/// Returns true if successful.
pub fn split_block(
    blocks: &mut Vec<ScheduleBlock>,
    index: usize,
    task_id: i64,
    task_name: String,
    day: String,
    duration_minutes: i32,
) -> bool {
    if index >= blocks.len() || duration_minutes <= 0 {
        return false;
    }

    let target = &blocks[index];
    if target.duration_minutes < duration_minutes {
        return false; // Selected block is too small
    }

    let old_duration = target.duration_minutes;
    let start_min = target.start_minute;
    let is_done = target.is_done;

    // Replace selected block with the task block
    blocks[index] = ScheduleBlock {
        id: None,
        task_id: Some(task_id),
        task_name,
        day: day.clone(),
        start_minute: start_min,
        duration_minutes,
        is_done: false,
    };

    // If there is leftover duration, insert a new "No plan" block
    if old_duration > duration_minutes {
        let leftover = old_duration - duration_minutes;
        blocks.insert(
            index + 1,
            ScheduleBlock {
                id: None,
                task_id: None,
                task_name: "No plan".to_string(),
                day,
                start_minute: start_min + duration_minutes,
                duration_minutes: leftover,
                is_done,
            },
        );
    }

    consolidate_blocks(blocks);
    true
}

/// Removes the scheduled task at `index` and turns it back into a "No plan" block.
pub fn delete_block(blocks: &mut Vec<ScheduleBlock>, index: usize) -> bool {
    if index >= blocks.len() {
        return false;
    }

    // Turn into "No plan"
    blocks[index].task_id = None;
    blocks[index].task_name = "No plan".to_string();
    blocks[index].is_done = false;

    consolidate_blocks(blocks);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_schedule() -> Vec<ScheduleBlock> {
        vec![
            ScheduleBlock {
                id: None,
                task_id: None,
                task_name: "No plan".to_string(),
                day: "2026-06-13".to_string(),
                start_minute: 0,
                duration_minutes: 1440,
                is_done: false,
            }
        ]
    }

    #[test]
    fn test_split() {
        let mut schedule = mock_schedule();
        assert!(split_block(&mut schedule, 0, 1, "Coding".to_string(), "2026-06-13".to_string(), 120));
        assert_eq!(schedule.len(), 2);
        assert_eq!(schedule[0].task_name, "Coding");
        assert_eq!(schedule[0].duration_minutes, 120);
        assert_eq!(schedule[1].task_name, "No plan");
        assert_eq!(schedule[1].duration_minutes, 1320);
        assert_eq!(schedule[1].start_minute, 120);
    }

    #[test]
    fn test_adjust() {
        let mut schedule = mock_schedule();
        split_block(&mut schedule, 0, 1, "Coding".to_string(), "2026-06-13".to_string(), 120);
        
        // Expand Coding by 30m
        assert!(adjust_block_duration(&mut schedule, 0, 30));
        assert_eq!(schedule[0].duration_minutes, 150);
        assert_eq!(schedule[1].duration_minutes, 1290);
        assert_eq!(schedule[1].start_minute, 150);
        
        // Contract Coding by 50m
        assert!(adjust_block_duration(&mut schedule, 0, -50));
        assert_eq!(schedule[0].duration_minutes, 100);
        assert_eq!(schedule[1].duration_minutes, 1340);
        assert_eq!(schedule[1].start_minute, 100);
    }
}
