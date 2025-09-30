pub mod service {
    use joy_macro::DisplayFromDebug;

    #[derive(Debug, DisplayFromDebug)]
    pub enum Error {
        Unknown,
    }

    pub type ServiceResult<T> = Result<T, Error>;

    impl<E> From<E> for Error
    where
        E: Into<anyhow::Error>,
    {
        fn from(_: E) -> Self {
            Self::Unknown
        }
    }
}
