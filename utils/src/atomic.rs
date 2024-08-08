use std::{
    ptr::null_mut,
    sync::atomic::{
        AtomicBool, AtomicI16, AtomicI32, AtomicI64, AtomicI8, AtomicIsize, AtomicPtr, AtomicU16,
        AtomicU32, AtomicU64, AtomicU8, AtomicUsize, Ordering,
    },
};

pub trait EasyAtomic {
    type Item;

    /// Update atomic value.
    ///
    /// ```no_run
    /// use std::sync::atomic::{AtomicU8, Ordering};
    /// use sync::atomic::EasyAtomic;
    ///
    /// impl EsayAtomic for AtomicU8 {
    ///     type Item = u8;
    ///
    ///     fn get(&self) -> Self::Item {
    ///         self.load(Ordering::Relaxed)
    ///     }
    ///
    ///     fn update(&self, value: Self::Item) -> Self::Item {
    ///         self.swap(value, Ordering::Relaxed)
    ///     }
    /// }
    /// ````
    fn update(&self, value: Self::Item) -> Self::Item;

    /// Get atomic value.
    ///
    /// ```no_run
    /// use std::sync::atomic::{AtomicU8, Ordering};
    /// use sync::atomic::EasyAtomic;
    ///
    /// impl EsayAtomic for AtomicU8 {
    ///     type Item = u8;
    ///
    ///     fn get(&self) -> Self::Item {
    ///         self.load(Ordering::Relaxed)
    ///     }
    ///
    ///     fn update(&self, value: Self::Item) -> Self::Item {
    ///         self.swap(value, Ordering::Relaxed)
    ///     }
    /// }
    /// ````
    fn get(&self) -> Self::Item;
}

macro_rules! easy_atomic {
    ($typed:ty, $item:ty) => {
        impl EasyAtomic for $typed {
            type Item = $item;

            fn get(&self) -> Self::Item {
                self.load(Ordering::Relaxed)
            }

            fn update(&self, value: Self::Item) -> Self::Item {
                self.swap(value, Ordering::Relaxed)
            }
        }
    };
}

easy_atomic!(AtomicI8, i8);
easy_atomic!(AtomicU8, u8);
easy_atomic!(AtomicI16, i16);
easy_atomic!(AtomicU16, u16);
easy_atomic!(AtomicI32, i32);
easy_atomic!(AtomicU32, u32);
easy_atomic!(AtomicI64, i64);
easy_atomic!(AtomicU64, u64);
easy_atomic!(AtomicBool, bool);
easy_atomic!(AtomicIsize, isize);
easy_atomic!(AtomicUsize, usize);

/// Atomized Option type.
pub struct AtomicOption<T>(AtomicPtr<T>);

impl<T> Default for AtomicOption<T> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<T> AtomicOption<T> {
    pub fn new(value: Option<T>) -> Self {
        Self(AtomicPtr::new(
            value
                .map(|v| Box::into_raw(Box::new(v)))
                .unwrap_or(null_mut()),
        ))
    }

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
    pub fn get(&self) -> Option<&'static T> {
        let value = self.0.load(Ordering::Relaxed);
        if !value.is_null() {
            Some(unsafe { &*value })
        } else {
            None
        }
    }

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
    pub fn swap(&self, value: Option<T>) -> Option<T> {
        let value = self.0.swap(
            value
                .map(|v| Box::into_raw(Box::new(v)))
                .unwrap_or(null_mut()),
            Ordering::Relaxed,
        );

        if !value.is_null() {
            Some(unsafe { *Box::from_raw(value) })
        } else {
            None
        }
    }

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
    pub fn is_none(&self) -> bool {
        self.0.load(Ordering::Relaxed).is_null()
    }

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
    pub fn is_some(&self) -> bool {
        !self.is_none()
    }
}

impl<T> Drop for AtomicOption<T> {
    fn drop(&mut self) {
        drop(self.swap(None))
    }
}
