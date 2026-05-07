#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod candidates;
mod fonts;
mod ipc;
mod models;
mod storage;
mod windows_integration;

use app::{AppMode, LinkInterceptorApp};
use ipc::IpcCommand;
use models::LaunchRequest;

fn main() -> eframe::Result<()> {
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
    let app_mode = if launch_request.is_some() {
        AppMode::InterceptWindow
    } else {
        AppMode::MainWindow
    };
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(app_mode.window_title())
            .with_inner_size(app_mode.initial_size())
            .with_min_inner_size(app_mode.min_size())
            .with_active(true),
        ..Default::default()
    };

    eframe::run_native(
        app_mode.window_title(),
        native_options,
        Box::new(move |cc| {
            fonts::configure(&cc.egui_ctx);
            Ok(Box::new(LinkInterceptorApp::new(
                launch_request.clone(),
                ipc_receiver,
            )))
        }),
    )
}
