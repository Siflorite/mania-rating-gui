pub mod bs;
pub mod callbacks;

use crate::MainWindow;
use slint::Weak;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tokio::sync::watch;

pub struct ScoreTileBase64 {
    pub index: i32,
    pub base64_string: String,
}

#[allow(unused)]
pub struct ThreadManager {
    handle: Option<JoinHandle<()>>,
    active: Arc<Mutex<bool>>,
}

impl ThreadManager {
    pub fn new() -> Self {
        Self {
            handle: None,
            active: Arc::new(Mutex::new(false)),
        }
    }

    pub fn stop_thread(&mut self) {
        if let Some(handle) = self.handle.take() {
            *self.active.lock().unwrap() = false;
            // handle.join().unwrap();
            println!("try stop thread");
            std::mem::drop(handle);
        }
    }

    pub fn start_thread<F>(&mut self, f: F, ui: Weak<MainWindow>)
    where
        F: Fn(Weak<MainWindow>, Arc<Mutex<bool>>) + Send + 'static,
    {
        self.stop_thread();
        let flag = self.active.clone();
        let t = std::thread::spawn(move || {
            f(ui, flag);
        });
        *self.active.lock().unwrap() = true;
        self.handle = Some(t);
    }

    pub fn is_active(&self) -> bool {
        *self.active.lock().unwrap() && self.handle.is_some()
    }
}

impl Default for ThreadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ThreadManager {
    fn drop(&mut self) {
        self.stop_thread();
    }
}

pub struct ThreadManagerAsync {
    handle: Option<tokio::task::JoinHandle<()>>,
    active: Arc<watch::Sender<bool>>,
}

impl ThreadManagerAsync {
    pub fn new() -> Self {
        let (sender, _) = watch::channel(false);
        Self {
            handle: None,
            active: Arc::new(sender),
        }
    }

    pub async fn stop_thread(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.active.send(false).unwrap();
            handle.abort();
            let _ = handle.await;
        }
    }

    pub async fn start_thread<F, Fut>(&mut self, f: F, ui: Weak<MainWindow>)
    where
        Fut: std::future::Future<Output = ()> + Send + 'static,
        F: Fn(Weak<MainWindow>, Arc<watch::Receiver<bool>>) -> Fut + Send + 'static,
    {
        self.stop_thread().await;
        let flag_receiver = self.active.subscribe();
        let t = tokio::spawn(f(ui, Arc::new(flag_receiver)));
        self.active.send(true).unwrap();
        self.handle = Some(t);
    }

    pub async fn is_active(&self) -> bool {
        *self.active.borrow() && self.handle.is_some()
    }
}

impl Default for ThreadManagerAsync {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ThreadManagerAsync {
    fn drop(&mut self) {
        // 注意：在Drop中不能直接调用async函数
        // 可以使用block_on，但不推荐在生产代码中这样做
        // tokio::runtime::Handle::current().block_on(async {
        //     self.stop_thread().await;
        // });
        if let Some(handle) = self.handle.take() {
            self.active.send(false).unwrap();
            handle.abort(); // 只标记，不等待
        }
    }
}
