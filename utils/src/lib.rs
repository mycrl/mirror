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
pub mod logger;
pub mod strings;
