#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod candidates;
mod ipc;
mod models;
mod native_window;
mod storage;
mod windows_integration;

use app::LinkInterceptorApp;
use ipc::IpcCommand;
use models::LaunchRequest;

fn main() {
    let initial_url = std::env::args().nth(1);
    let ipc_command = initial_url
        .as_ref()
        .map(|url| IpcCommand::OpenIntercept { url: url.clone() })
        .unwrap_or(IpcCommand::ShowMain);
    if ipc::try_send(&ipc_command) {
        return;
    }

    let ipc_receiver = ipc::start_listener();
    if ipc_receiver.is_none() && ipc::try_send(&ipc_command) {
        return;
    }

    let launch_request = initial_url.map(LaunchRequest::new);
    LinkInterceptorApp::run(launch_request, ipc_receiver);
}
