use std::os::unix::fs::MetadataExt;

use rocket::{Build, Rocket};

use rocket_prometheus::PrometheusMetrics;
use rocket_prometheus::prometheus::PullingGauge;

use crate::paste_id::PasteId;

fn stored_files() -> f64 {
    let dir = PasteId::file_root_dir();
    std::fs::read_dir(dir)
        .map(|res| res.count() as f64)
        .unwrap_or(-1f64)
}

fn total_file_size() -> f64 {
    let dir = PasteId::file_root_dir();

    let files = std::fs::read_dir(dir)
        .ok()
        .expect("Failed to read pastebin store dir");
    let size: u64 = files
        .flatten()
        .map(|file| file.metadata())
        .flatten()
        .map(|meta| meta.size())
        .sum();

    size as f64
}

pub(crate) trait AttachMetrics {
    fn add_metrics(self) -> Self;
}

fn make(
    name: &'static str,
    desc: &'static str,
    fun: Box<dyn Fn() -> f64 + Send + Sync>,
) -> Box<PullingGauge> {
    Box::new(
        PullingGauge::new(name, desc, fun)
            .expect("Failed to create pastebin_stored_files openmetrics gauge"),
    )
}

fn register(prom: &PrometheusMetrics, gauge: Box<PullingGauge>) {
    prom.registry()
        .register(gauge)
        .expect("Failed to register prometheus gauge");
}

impl AttachMetrics for Rocket<Build> {
    fn add_metrics(self) -> Self {
        let prometheus = PrometheusMetrics::new();

        register(
            &prometheus,
            make(
                "pastebin_stored_files",
                "Amount of files currently stored in the pastebin",
                Box::new(stored_files),
            ),
        );
        register(
            &prometheus,
            make(
                "pastebin_total_file_size",
                "Total size of stored files, in bytes",
                Box::new(total_file_size),
            ),
        );

        self.attach(prometheus.clone())
            .mount("/metrics", prometheus)
    }
}
