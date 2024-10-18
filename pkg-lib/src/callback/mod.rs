#[cfg(feature = "indicatif")]
pub use self::indicatif::IndicatifCallback;
#[cfg(feature = "indicatif")]
mod indicatif;

pub trait Callback {
    fn start_download(&mut self, length: u64, file: &str);
    fn increment_downloaded(&mut self, downloaded: usize);
    fn end_download(&mut self);

    fn conflict(&mut self) {}

    // todo: add error handeling
    fn error(&mut self) {}
}
