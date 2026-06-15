#![allow(clippy::needless_range_loop)]

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    pub id: i64,
    pub name: String,
    pub default_duration: i32, // in minutes
    pub notes: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduleBlock {
    pub id: Option<i64>,
    pub task_ids: Vec<i64>,   // Empty represents "No plan"
    pub task_name: String,    // Display name: e.g., "Coding + Podcast" or "No plan"
    pub day: String,          // YYYY-MM-DD
    pub start_minute: i32,    // 0 to 1439
    pub duration_minutes: i32,
    pub is_done: bool,
    pub notes: String,
}

/// Dynamic file logging centralized under the sovereign .nullvector path.
pub fn log_msg(level: &str, msg: &str) {
    let log_path = "C:\\Users\\Israel\\.nullvector\\lockstep.log";
    if let Some(parent) = std::path::Path::new(log_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let _ = writeln!(file, "[{}] [{}] {}", timestamp, level, msg);
    }
}

pub fn log_info(msg: &str) {
    log_msg("INFO", msg);
}

pub fn log_error(msg: &str) {
    log_msg("ERROR", msg);
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
        if blocks[i - 1].task_ids.is_empty() && blocks[i].task_ids.is_empty() {
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
        log_error(&format!("Adjustment rejected: Block duration cannot be zero or negative (idx: {}, delta: {})", index, delta_minutes));
        return false;
    }

    // Identify target blocks downstream to absorb the opposite delta
    if delta_minutes > 0 {
        // Find downstream "No plan" blocks to shrink
        let mut target_indices = Vec::new();
        for i in (index + 1)..blocks.len() {
            if blocks[i].task_ids.is_empty() {
                target_indices.push(i);
            }
        }

        // If no "No plan" block exists downstream, fall back to the last block
        if target_indices.is_empty() && index + 1 < blocks.len() {
            target_indices.push(blocks.len() - 1);
        }

        if target_indices.is_empty() {
            log_error("Adjustment rejected: No downstream buffer blocks available to contract.");
            return false;
        }

        // Calculate available buffer size
        let total_available: i32 = target_indices.iter().map(|&i| blocks[i].duration_minutes).sum();
        if total_available < delta_minutes {
            log_error(&format!("Adjustment rejected: Downstream buffer ({}m) is smaller than requested expansion ({}m)", total_available, delta_minutes));
            return false;
        }

        // Absorb expansion from buffer blocks
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
        // Prefer expanding the first downstream "No plan" block.
        let mut target_idx = None;
        for i in (index + 1)..blocks.len() {
            if blocks[i].task_ids.is_empty() {
                target_idx = Some(i);
                break;
            }
        }

        let t_idx = target_idx.unwrap_or_else(|| {
            if index + 1 < blocks.len() {
                blocks.len() - 1
            } else {
                index
            }
        });

        if t_idx == index {
            log_error("Adjustment rejected: Cannot adjust the last block alone when no buffer exists.");
            return false;
        }

        blocks[index].duration_minutes += delta_minutes;
        blocks[t_idx].duration_minutes -= delta_minutes; // expand (subtract negative is addition)
    }

    // Cleanup 0 duration blocks
    blocks.retain(|b| b.duration_minutes > 0 || b.task_ids.is_empty());
    consolidate_blocks(blocks);
    log_info(&format!("Adjusted block {} duration by {}m successfully.", index, delta_minutes));
    true
}

/// Splits the block at `index` (usually "No plan") to insert a scheduled task of `duration_minutes`.
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
        log_error(&format!("Split rejected: Block duration ({}m) is smaller than requested task duration ({}m)", target.duration_minutes, duration_minutes));
        return false;
    }

    let old_duration = target.duration_minutes;
    let start_min = target.start_minute;
    let is_done = target.is_done;
    let block_notes = target.notes.clone();

    // Replace selected block with the task block
    blocks[index] = ScheduleBlock {
        id: None,
        task_ids: vec![task_id],
        task_name,
        day: day.clone(),
        start_minute: start_min,
        duration_minutes,
        is_done: false,
        notes: block_notes.clone(),
    };

    // If there is leftover duration, insert a new "No plan" block
    if old_duration > duration_minutes {
        let leftover = old_duration - duration_minutes;
        blocks.insert(
            index + 1,
            ScheduleBlock {
                id: None,
                task_ids: Vec::new(),
                task_name: "No plan".to_string(),
                day,
                start_minute: start_min + duration_minutes,
                duration_minutes: leftover,
                is_done,
                notes: String::new(),
            },
        );
    }

    consolidate_blocks(blocks);
    log_info(&format!("Split block {} to insert task ID {} ({}m)", index, task_id, duration_minutes));
    true
}

/// Appends a task template to an existing block's schedule (doubling up).
pub fn append_task_to_block(
    blocks: &mut [ScheduleBlock],
    index: usize,
    task_id: i64,
    task_name: String,
) -> bool {
    if index >= blocks.len() {
        return false;
    }

    let block = &mut blocks[index];
    if block.task_ids.contains(&task_id) {
        log_info(&format!("Task ID {} already present in block {}. Skipping append.", task_id, index));
        return false; // Prevent duplicates in the same block
    }

    if block.task_ids.is_empty() {
        // If the block is "No plan", it becomes a single task block
        block.task_ids = vec![task_id];
        block.task_name = task_name;
    } else {
        // Double up (multi-task)
        block.task_ids.push(task_id);
        block.task_name = format!("{} + {}", block.task_name, task_name);
    }

    log_info(&format!("Doubled up block {} by appending task ID {}", index, task_id));
    true
}

/// Removes the scheduled tasks at `index` and turns it back into a "No plan" block.
pub fn delete_block(blocks: &mut Vec<ScheduleBlock>, index: usize) -> bool {
    if index >= blocks.len() {
        return false;
    }

    blocks[index].task_ids.clear();
    blocks[index].task_name = "No plan".to_string();
    blocks[index].is_done = false;
    // We clear the notes too upon deleting the task schedule block
    blocks[index].notes.clear();

    consolidate_blocks(blocks);
    log_info(&format!("Deleted task entry at block index {} and reverted to No Plan", index));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_schedule() -> Vec<ScheduleBlock> {
        vec![
            ScheduleBlock {
                id: None,
                task_ids: Vec::new(),
                task_name: "No plan".to_string(),
                day: "2026-06-13".to_string(),
                start_minute: 0,
                duration_minutes: 1440,
                is_done: false,
                notes: String::new(),
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
        assert_eq!(schedule[0].task_ids, vec![1]);
        assert_eq!(schedule[1].task_name, "No plan");
        assert_eq!(schedule[1].duration_minutes, 1320);
    }

    #[test]
    fn test_double_up() {
        let mut schedule = mock_schedule();
        split_block(&mut schedule, 0, 1, "Coding".to_string(), "2026-06-13".to_string(), 120);
        assert!(append_task_to_block(&mut schedule, 0, 2, "Music".to_string()));
        assert_eq!(schedule[0].task_name, "Coding + Music");
        assert_eq!(schedule[0].task_ids, vec![1, 2]);
    }
}
