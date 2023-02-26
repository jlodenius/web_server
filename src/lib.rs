use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
};

// Type alias
type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Drop the sender first, this closes the channel
        // and makes all calls to recv return an error, which will exit
        // an otherwise infinite loop in the worker implementation
        drop(self.sender.take());

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            // thread.join takes ownership of thread and we only have a mutable
            // borrow. So we put it in an Option and use the take fn to take
            // the thread out of its Worker
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        let mut workers = Vec::with_capacity(size);

        // Shadow the receiver in an Arc Mutex to allow multiple owners
        // in different threads
        let receiver = Arc::new(Mutex::new(receiver));

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        // using as_ref to convert Option<T> to Option<&T>
        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => {
                    println!("Worker {id} got a job; executing.");
                    job();
                }
                Err(_) => {
                    println!("Worker {id} disconnected; shutting down.");
                    // Break out of the loop as soon as we get an error.
                    // i.e. on graceful shutdown from the drop implementation
                    // of our ThreadPool
                    break;
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}
