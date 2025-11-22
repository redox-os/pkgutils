#[cfg(feature = "indicatif")]
pub use self::indicatif::IndicatifCallback;
pub use self::plain::PlainCallback;
#[cfg(feature = "indicatif")]
mod indicatif;
mod plain;

pub trait Callback {
    fn start_download(&mut self, length: u64, file: &str);
    fn increment_downloaded(&mut self, downloaded: u64);
    fn end_download(&mut self);

    fn conflict(&mut self) {}

    // todo: add error handeling
    fn error(&mut self) {}
}
