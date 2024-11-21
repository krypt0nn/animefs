use std::collections::VecDeque;

use flume::{Sender, Receiver};

use crate::prelude::*;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilesystemTaskPriority {
    /// Task will be executed before all the other operations.
    High,

    #[default]
    /// Task will be executed after earlier operations
    /// were finished.
    Normal,

    /// Task will be executed when all the other
    /// operations were performed.
    Low
}

#[derive(Debug, Clone)]
pub enum FilesystemSchedulerTask {
    /// Push task to the scheduler.
    PushTask {
        task: FilesystemTask,
        priority: FilesystemTaskPriority
    },

    /// Poll next task from the scheduler.
    PollTask(Sender<FilesystemTask>)
}

#[derive(Debug, Clone)]
pub struct FilesystemTasksScheduler {
    tasks_high: VecDeque<FilesystemTask>,
    tasks_normal: VecDeque<FilesystemTask>,
    tasks_low: VecDeque<FilesystemTask>,

    tasks_polls: VecDeque<Sender<FilesystemTask>>,

    listener: Receiver<FilesystemSchedulerTask>
}

impl FilesystemTasksScheduler {
    /// Create new pair of tasks scheduler and handler.
    pub fn new() -> (Self, FilesystemTasksHandler) {
        let (sender, listener) = flume::unbounded();

        let handler = FilesystemTasksHandler::new(sender);

        let scheduler = Self {
            tasks_low: VecDeque::new(),
            tasks_normal: VecDeque::new(),
            tasks_high: VecDeque::new(),

            tasks_polls: VecDeque::new(),

            listener
        };

        (scheduler, handler)
    }

    #[inline]
    /// Spawn new thread and run scheduler updates in a loop.
    pub fn daemonize(mut self) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            loop {
                if !self.update() {
                    break;
                }
            }
        })
    }

    /// Listen for incoming tasks and put them in
    /// appropriate queues using their priority.
    ///
    /// Return false if all the tasks handlers were closed.
    pub fn update(&mut self) -> bool {
        loop {
            match self.listener.try_recv() {
                Ok(task) => {
                    match task {
                        FilesystemSchedulerTask::PushTask { task, priority } => self.push(task, priority),
                        FilesystemSchedulerTask::PollTask(sender) => self.tasks_polls.push_back(sender)
                    }
                }

                Err(flume::TryRecvError::Disconnected) => return false,
                Err(flume::TryRecvError::Empty) => break
            }
        }

        // If somebody requested tasks from the scheduler.
        if !self.tasks_polls.is_empty() {
            let mut last_task = None;

            // Take task read requester according to the queue.
            while let Some(sender) = self.tasks_polls.pop_front() {
                // Immediately skip it if connection is closed.
                if sender.is_disconnected() {
                    continue;
                }

                // Take next scheduled task if it's not taken already.
                if last_task.is_none() {
                    last_task = self.poll();
                }

                // Check the task's state. If there's no tasks in the scheduler -
                // skip execution.
                match last_task.take() {
                    // We've polled a task from the scheduler and need to send it.
                    Some((task, priority)) => {
                        // If task wasn't sent (how if sender is connected?) - remember it
                        // and try to send again to the next request.
                        if sender.send(task.clone()).is_err() {
                            last_task = Some((task, priority));

                            continue;
                        }
                    }

                    // last_task is None so we don't need to call push_front later.
                    None => return true
                }
            }

            // If the last scheduler task wasn't sent to anybody - keep it in the scheduler.
            if let Some((task, priority)) = last_task {
                self.push_front(task, priority);
            }
        }

        true
    }

    /// Push task to the scheduler.
    pub fn push(&mut self, task: FilesystemTask, priority: FilesystemTaskPriority) {
        match priority {
            FilesystemTaskPriority::High   => self.tasks_high.push_back(task),
            FilesystemTaskPriority::Normal => self.tasks_normal.push_back(task),
            FilesystemTaskPriority::Low    => self.tasks_low.push_back(task)
        }
    }

    /// Push task to the scheduler at the first place in the queue.
    pub fn push_front(&mut self, task: FilesystemTask, priority: FilesystemTaskPriority) {
        match priority {
            FilesystemTaskPriority::High   => self.tasks_high.push_front(task),
            FilesystemTaskPriority::Normal => self.tasks_normal.push_front(task),
            FilesystemTaskPriority::Low    => self.tasks_low.push_front(task)
        }
    }

    /// Try to poll a task from the scheduler.
    pub fn poll(&mut self) -> Option<(FilesystemTask, FilesystemTaskPriority)> {
        self.tasks_high.pop_front()
            .map(|task| (task, FilesystemTaskPriority::High))
            .or_else(|| {
                self.tasks_normal.pop_front()
                    .map(|task| (task, FilesystemTaskPriority::Normal))
            })
            .or_else(|| {
                self.tasks_low.pop_front()
                    .map(|task| (task, FilesystemTaskPriority::Low))
            })
    }
}
