//! # RefCells for Neotron OS.
//!
//! Like the `RefCell` in the standard library, except that it's thread-safe
//! and uses the BIOS critical section to make it so.

// ===========================================================================
// Modules and Imports
// ===========================================================================

use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

// ===========================================================================
// Global Variables
// ===========================================================================

// None

// ===========================================================================
// Macros
// ===========================================================================

// None

// ===========================================================================
// Public types
// ===========================================================================

/// Indicates a failure to lock the refcell because it was already locked.
#[derive(Debug)]
pub struct LockError;

/// A cell that gives you references, and is thread-safe.
///
/// Uses the BIOS to ensure thread-safety whilst checking if the lock is taken
/// or not.
pub struct CsRefCell<T> {
    inner: UnsafeCell<T>,
    locked: AtomicBool,
}

impl<T> CsRefCell<T> {
    /// Create a new cell.
    pub const fn new(value: T) -> CsRefCell<T> {
        CsRefCell {
            inner: UnsafeCell::new(value),
            locked: AtomicBool::new(false),
        }
    }

    /// Try and do something with the lock.
    pub fn with<F, U>(&self, f: F) -> Result<U, LockError>
    where
        F: FnOnce(&mut CsRefCellGuard<T>) -> U,
    {
        let mut guard = self.try_lock()?;
        let result = f(&mut guard);
        drop(guard);
        Ok(result)
    }

    /// Lock the cell.
    ///
    /// If you can't lock it (because it is already locked), this function will panic.
    pub fn lock(&self) -> CsRefCellGuard<T> {
        self.try_lock().unwrap()
    }

    /// Try and grab the lock.
    ///
    /// It'll fail if it's already been taken.
    ///
    /// It'll panic if the global lock is in a bad state, or you try and
    /// re-enter this function from an interrupt whilst the global lock is held.
    /// Don't do that.
    pub fn try_lock(&self) -> Result<CsRefCellGuard<T>, LockError> {
        let api = crate::API.get();

        if (api.compare_and_swap_bool)(&self.locked, false, true) {
            // succesfully swapped `false` for `true`
            core::sync::atomic::fence(Ordering::Acquire);
            Ok(CsRefCellGuard { parent: self })
        } else {
            // cell is already locked
            Err(LockError)
        }
    }
}

/// Mark our type as thread-safe.
///
/// # Safety
///
/// We use the BIOS critical sections to control access. Thus it is now
/// thread-safe.
unsafe impl<T> Sync for CsRefCell<T> {}

/// Represents an active borrow of a [`CsRefCell`].
pub struct CsRefCellGuard<'a, T> {
    parent: &'a CsRefCell<T>,
}

impl<'a, T> Deref for CsRefCellGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let ptr = self.parent.inner.get();
        unsafe { &*ptr }
    }
}

impl<'a, T> DerefMut for CsRefCellGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let ptr = self.parent.inner.get();
        unsafe { &mut *ptr }
    }
}

impl<'a, T> Drop for CsRefCellGuard<'a, T> {
    fn drop(&mut self) {
        // We hold this refcell guard exclusively, so this can't race
        self.parent.locked.store(false, Ordering::Release);
    }
}

// ===========================================================================
// Private types
// ===========================================================================

// None

// ===========================================================================
// Private functions
// ===========================================================================

// None

// ===========================================================================
// Public functions
// ===========================================================================

// None

// ===========================================================================
// Tests
// ===========================================================================

// None

// ===========================================================================
// End of file
// ===========================================================================
