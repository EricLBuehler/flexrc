#![cfg(test)]

use std::thread;

use crate::{FlexRc, FlexRcImpl, FlexRcImplSend};

#[test]
fn unsync_box() {
    let flex = <FlexRc<_, _> as FlexRcImpl<_>>::new(Box::new(5));
    let _other = flex.clone();
}

#[test]
fn sync_box() {
    let flex = <FlexRc<_, _> as FlexRcImplSend<_>>::new(Box::new(5));

    let other = flex.clone();
    let _thread = thread::spawn(move || {
        let _cloned = other.clone();
    });
}

/*
#[test]
fn immortal_box() {
    use crate::FlexRcImplImmortal;

    let flex = <FlexRc<_, _> as FlexRcImplImmortal<_>> ::new(Box::new(5));

    let other = flex.clone();
    let _thread = thread::spawn(move || {
        let _cloned = other.clone();
    });
}
*/
