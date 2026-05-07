#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod candidates;
mod fonts;
mod ipc;
mod models;
mod storage;
mod windows_integration;

use app::LinkInterceptorApp;
use ipc::IpcCommand;
use models::LaunchRequest;
use std::sync::{Arc, Mutex};

fn main() -> iced::Result {
    let initial_url = std::env::args().nth(1);
    let ipc_command = initial_url
        .as_ref()
        .map(|url| IpcCommand::OpenIntercept { url: url.clone() })
        .unwrap_or(IpcCommand::ShowMain);
    if ipc::try_send(&ipc_command) {
        return Ok(());
    }

    let ipc_receiver = ipc::start_listener();
    if ipc_receiver.is_none() && ipc::try_send(&ipc_command) {
        return Ok(());
    }

    let launch_request = initial_url.map(LaunchRequest::new);
    let ipc_receiver = Arc::new(Mutex::new(ipc_receiver));
    let settings = fonts::settings();

    iced::daemon(
        move || {
            let ipc_receiver = ipc_receiver
                .lock()
                .ok()
                .and_then(|mut receiver| receiver.take());
            LinkInterceptorApp::boot(launch_request.clone(), ipc_receiver)
        },
        LinkInterceptorApp::update,
        LinkInterceptorApp::view,
    )
    .title(LinkInterceptorApp::title)
    .subscription(LinkInterceptorApp::subscription)
    .settings(settings)
    .run()
}
