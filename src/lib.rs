use std::{
    alloc::Layout,
    cell::Cell,
    fmt::{Debug, Display},
    hash::Hash,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::Deref,
    ptr::{self, addr_of_mut, NonNull},
    sync::atomic::{fence, AtomicUsize, Ordering},
};

mod test;

/// We do not use an enum because most accesses will use compile-time type state to ensure soundness.
/// This is used purely during drop.
enum Discriminant {
    Immortal,
    Send,
    Simple,
}

#[repr(C)]
union FlexRcRefcount {
    atomic: ManuallyDrop<AtomicUsize>,
    simple: ManuallyDrop<Cell<usize>>,
    immortal: (),
}

struct FlexRcBox<T: ?Sized> {
    refcount: FlexRcRefcount,
    discriminant: Discriminant,
    data: T,
}

pub struct FlexRc<T: ?Sized, M> {
    ptr: NonNull<FlexRcBox<T>>,
    _marker: PhantomData<M>,
}

pub struct FlexRcImmortal;
pub struct FlexRcSend;
pub struct FlexRcSimple;

pub trait FlexRcImplImmortal<T: ?Sized> {
    fn new(data: T) -> Self;
    fn clone(&self) -> Self;
}

pub trait FlexRcImplImmortalDefault<T: ?Sized + Default>: FlexRcImplImmortal<T> {
    fn default() -> Self;
}

pub trait FlexRcImplSend<T: ?Sized + Send> {
    fn new(data: T) -> Self;
    fn clone(&self) -> Self;
}

pub trait FlexRcImplSendDefault<T: ?Sized + Send + Default>: FlexRcImplSend<T> {
    fn default() -> Self;
}

pub trait FlexRcImpl<T: ?Sized> {
    fn new(data: T) -> Self;
    fn clone(&self) -> Self;
}

pub trait FlexRcImplDefault<T: ?Sized + Default>: FlexRcImpl<T> {
    fn default() -> Self;
}

impl<T: Send> FlexRcImplSend<T> for FlexRc<T, FlexRcSend> {
    /// Create a new, `send`, `FlexRc`.
    fn new(data: T) -> Self {
        let refcount = FlexRcRefcount {
            atomic: ManuallyDrop::new(AtomicUsize::new(1)),
        };
        let ptr = FlexRcBox {
            refcount,
            discriminant: Discriminant::Send,
            data,
        };
        let ptr = NonNull::from(Box::leak(Box::new(ptr)));
        Self {
            ptr,
            _marker: PhantomData,
        }
    }

    fn clone(&self) -> Self {
        // SAFETY: we know have this specific field, we are in this impl.
        let refcount = unsafe { &self.inner().refcount.atomic };
        // SOUNDNESS: new references may only be formed from an existing reference.
        refcount.fetch_add(1, Ordering::Relaxed);
        Self {
            ptr: self.ptr,
            _marker: self._marker,
        }
    }
}

impl<T: Send + Default> FlexRcImplSendDefault<T> for FlexRc<T, FlexRcSend> {
    fn default() -> Self {
        <FlexRc<_, _> as FlexRcImplSend<_>>::new(Default::default())
    }
}

unsafe impl<T: ?Sized> Send for FlexRc<T, FlexRcSend> {}
unsafe impl<T: ?Sized> Sync for FlexRc<T, FlexRcSend> {}

impl<T> FlexRcImpl<T> for FlexRc<T, FlexRcSimple> {
    /// Create a new, unsync, `FlexRc`.
    fn new(data: T) -> Self {
        let refcount = FlexRcRefcount {
            simple: ManuallyDrop::new(Cell::new(1)),
        };
        let ptr = FlexRcBox {
            refcount,
            discriminant: Discriminant::Simple,
            data,
        };
        let ptr = NonNull::from(Box::leak(Box::new(ptr)));
        Self {
            ptr,
            _marker: PhantomData,
        }
    }

    fn clone(&self) -> Self {
        // SAFETY: we know have this specific field, we are in this impl.
        let refcount = unsafe { &self.inner().refcount.simple };
        let refcount_new = refcount.get() + 1;
        refcount.set(refcount_new);
        Self {
            ptr: self.ptr,
            _marker: self._marker,
        }
    }
}

impl<T: Default> FlexRcImplDefault<T> for FlexRc<T, FlexRcSimple> {
    fn default() -> Self {
        <FlexRc<_, _> as FlexRcImpl<_>>::new(Default::default())
    }
}

impl<T> FlexRcImplImmortal<T> for FlexRc<T, FlexRcImmortal> {
    /// Create a new, immortal, `FlexRc`.
    fn new(data: T) -> Self {
        let refcount = FlexRcRefcount { immortal: () };
        let ptr = FlexRcBox {
            refcount,
            discriminant: Discriminant::Immortal,
            data,
        };
        let ptr = NonNull::from(Box::leak(Box::new(ptr)));
        Self {
            ptr,
            _marker: PhantomData,
        }
    }

    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            _marker: self._marker,
        }
    }
}

impl<T: Default> FlexRcImplImmortalDefault<T> for FlexRc<T, FlexRcImmortal> {
    fn default() -> Self {
        <FlexRc<_, _> as FlexRcImplImmortal<_>>::new(Default::default())
    }
}

unsafe impl<T: ?Sized> Send for FlexRc<T, FlexRcImmortal> {}
unsafe impl<T: ?Sized> Sync for FlexRc<T, FlexRcImmortal> {}

impl<T: ?Sized, M> FlexRc<T, M> {
    fn inner(&self) -> &FlexRcBox<T> {
        // SAFETY: The pointer is valid as long as the FlexRc is alive.
        unsafe { self.ptr.as_ref() }
    }

    unsafe fn drop_slow(&mut self) {
        ptr::drop_in_place(addr_of_mut!((*self.ptr.as_ptr()).data));

        let layout = Layout::for_value(unsafe { &*self.ptr.as_ptr() });
        unsafe {
            std::alloc::dealloc(self.ptr.as_ptr().cast(), layout);
        }
    }
}

impl<T: ?Sized, M> Drop for FlexRc<T, M> {
    fn drop(&mut self) {
        match self.inner().discriminant {
            Discriminant::Immortal => {}
            Discriminant::Send => {
                // SAFETY: we know have this specific field, we have the discriminant.
                let refcount = unsafe { &self.inner().refcount.atomic };
                // SOUNDNESS: new references may only be formed from an existing reference.
                if refcount.fetch_sub(1, Ordering::Release) != 1 {
                    return;
                }

                // See the Arc::drop notes.
                // This ensures that we are absolutely clear about our refcount and how it relates
                // to ensuring that they happen before we drop.
                fence(Ordering::Acquire);

                unsafe { self.drop_slow() };
            }
            Discriminant::Simple => {
                // SAFETY: we know have this specific field, we have the discriminant.
                let refcount = unsafe { &self.inner().refcount.simple };
                let refcount_new = refcount.get() - 1;
                refcount.set(refcount_new);
                if refcount_new != 0 {
                    return;
                }

                unsafe { self.drop_slow() };
            }
        }
    }
}

impl<T: ?Sized, M> Deref for FlexRc<T, M> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner().data
    }
}

impl<T: ?Sized, M> AsRef<T> for FlexRc<T, M> {
    fn as_ref(&self) -> &T {
        self
    }
}

impl<T: ?Sized + Display, M> Display for FlexRc<T, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized + Debug, M> Debug for FlexRc<T, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized + Eq, M> Eq for FlexRc<T, M> {}

impl<T: ?Sized + PartialEq, M> PartialEq for FlexRc<T, M> {
    fn eq(&self, other: &Self) -> bool {
        (**self).eq(other)
    }
}

impl<T: ?Sized + Ord, M> Ord for FlexRc<T, M> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (**self).cmp(other)
    }
}

impl<T: ?Sized + PartialOrd, M> PartialOrd for FlexRc<T, M> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        (**self).partial_cmp(other)
    }
}

impl<T: ?Sized + Hash, M> Hash for FlexRc<T, M> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (**self).hash(state)
    }
}
