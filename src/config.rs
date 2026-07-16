#[derive(Clone, Debug)]
pub struct Config {
    pub bind: String,
    pub fps: u32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8080".into(),
            fps: 30,
        }
    }
}
