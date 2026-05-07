mod app;
mod candidates;
mod fonts;
mod models;
mod storage;
mod windows_integration;

use app::LinkInterceptorApp;
use models::LaunchRequest;

fn main() -> eframe::Result<()> {
    let initial_url = std::env::args().nth(1);
    let launch_request = initial_url.map(LaunchRequest::new);
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Link Interceptor")
            .with_inner_size([1024.0, 720.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Link Interceptor",
        native_options,
        Box::new(move |cc| {
            fonts::configure(&cc.egui_ctx);
            Ok(Box::new(LinkInterceptorApp::new(launch_request.clone())))
        }),
    )
}
