use flume::Sender;

use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct FilesystemTasksHandler {
    sender: Sender<FilesystemSchedulerTask>
}

impl FilesystemTasksHandler {
    #[inline]
    pub fn new(sender: Sender<FilesystemSchedulerTask>) -> Self {
        Self {
            sender
        }
    }

    /// Send filesystem task to the scheduler with specified priority.
    pub fn send(&self, task: FilesystemTask, priority: FilesystemTaskPriority) -> anyhow::Result<()> {
        self.sender.send(FilesystemSchedulerTask::PushTask {
            task,
            priority
        })?;

        Ok(())
    }

    /// Send filesystem task to the scheduler with highest priority.
    pub fn send_high(&self, task: FilesystemTask) -> anyhow::Result<()> {
        self.send(task, FilesystemTaskPriority::High)
    }

    /// Send filesystem task to the scheduler with normal priority.
    pub fn send_normal(&self, task: FilesystemTask) -> anyhow::Result<()> {
        self.send(task, FilesystemTaskPriority::Normal)
    }

    /// Send filesystem task to the scheduler with lowest priority.
    pub fn send_low(&self, task: FilesystemTask) -> anyhow::Result<()> {
        self.send(task, FilesystemTaskPriority::Low)
    }

    /// Poll filesystem task from the scheduler.
    pub fn poll(&self) -> anyhow::Result<FilesystemTask> {
        let (send, recv) = flume::bounded(1);

        self.sender.send(FilesystemSchedulerTask::PollTask(send))?;

        Ok(recv.recv()?)
    }
}
