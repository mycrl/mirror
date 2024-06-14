/// # Example
///
/// ```no_run
/// use sync::atomic::AtomicOption;
///
/// let opt = AtomicOption::<u8>::new(None);
/// assert_eq!(opt.get().is_none(), true);
/// assert!(opt.is_none());
/// assert!(!opt.is_some());
///
/// let b = opt.swap(Some(1));
/// assert_eq!(b, None);
/// assert_eq!(opt.get().is_none(), false);
/// assert!(!opt.is_none());
/// assert!(opt.is_some());
/// ```
pub mod atomic;
pub mod frame;
pub mod logger;
pub mod strings;

pub fn jump_current_exe_dir() -> anyhow::Result<()> {
    let mut path = std::env::current_exe()?;
    path.pop();
    std::env::set_current_dir(path)?;

    Ok(())
}
