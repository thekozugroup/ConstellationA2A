use std::collections::HashMap;

use tracing::{debug, info, warn};

use crate::error::{ConstellationError, Result};
use crate::message::{TaskResult, TaskStatus};

/// In-memory record for a tracked task.
#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub task_id: String,
    pub task_type: String,
    pub status: TaskStatus,
    pub payload: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub room_id: String,
}

/// Manages the lifecycle of tasks created by or assigned to this agent.
#[derive(Debug, Default)]
pub struct TaskManager {
    tasks: HashMap<String, TaskRecord>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    /// Register a new task. Returns the task ID.
    pub fn create(
        &mut self,
        task_id: impl Into<String>,
        task_type: impl Into<String>,
        payload: serde_json::Value,
        room_id: impl Into<String>,
    ) -> String {
        let task_id = task_id.into();
        info!(task_id = %task_id, "Creating task");
        let record = TaskRecord {
            task_id: task_id.clone(),
            task_type: task_type.into(),
            status: TaskStatus::Pending,
            payload,
            result: None,
            room_id: room_id.into(),
        };
        self.tasks.insert(task_id.clone(), record);
        task_id
    }

    /// Update the status of an existing task.
    pub fn update_status(&mut self, task_id: &str, status: TaskStatus) -> Result<()> {
        let record = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| ConstellationError::Task(format!("task not found: {task_id}")))?;
        debug!(task_id = %task_id, old = ?record.status, new = ?status, "Updating task status");
        record.status = status;
        Ok(())
    }

    /// Mark a task as completed with a result.
    pub fn complete(&mut self, task_id: &str, result: TaskResult) -> Result<()> {
        let record = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| ConstellationError::Task(format!("task not found: {task_id}")))?;
        info!(task_id = %task_id, status = ?result.status, "Completing task");
        record.status = result.status;
        record.result = Some(result.result_data);
        Ok(())
    }

    /// Get the current status of a task.
    pub fn get_status(&self, task_id: &str) -> Option<TaskStatus> {
        self.tasks.get(task_id).map(|r| r.status)
    }

    /// Get the full record for a task.
    pub fn get(&self, task_id: &str) -> Option<&TaskRecord> {
        self.tasks.get(task_id)
    }

    /// List all tasks with a given status.
    pub fn list_by_status(&self, status: TaskStatus) -> Vec<&TaskRecord> {
        self.tasks.values().filter(|r| r.status == status).collect()
    }

    /// List all pending tasks.
    pub fn list_pending(&self) -> Vec<&TaskRecord> {
        self.list_by_status(TaskStatus::Pending)
    }

    /// Remove a completed or failed task from tracking.
    pub fn remove(&mut self, task_id: &str) -> Option<TaskRecord> {
        let removed = self.tasks.remove(task_id);
        if removed.is_some() {
            debug!(task_id = %task_id, "Removed task from tracker");
        } else {
            warn!(task_id = %task_id, "Attempted to remove non-existent task");
        }
        removed
    }

    /// Total number of tracked tasks.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_task_lifecycle() {
        let mut mgr = TaskManager::new();
        let id = mgr.create("t1", "analysis", json!({"file": "data.csv"}), "!room:test");

        assert_eq!(mgr.get_status(&id), Some(TaskStatus::Pending));
        assert_eq!(mgr.list_pending().len(), 1);

        mgr.update_status(&id, TaskStatus::InProgress).unwrap();
        assert_eq!(mgr.get_status(&id), Some(TaskStatus::InProgress));
        assert!(mgr.list_pending().is_empty());

        let result = TaskResult::success(&id, json!({"answer": 42}));
        mgr.complete(&id, result).unwrap();
        assert_eq!(mgr.get_status(&id), Some(TaskStatus::Completed));
    }
}
