#![cfg(test)]

use std::thread;

use crate::{FlexRc, FlexRcImpl, FlexRcImplMake, FlexRcImplSend, FlexRcImplSendMake};

#[test]
fn simple_box() {
    let flex = <FlexRc<_, _> as FlexRcImpl<_>>::new(Box::new(5));
    let _other = flex.clone();
}

#[test]
fn simple_box_make_send() {
    let flex = <FlexRc<_, _> as FlexRcImpl<_>>::new(0);
    let normal = flex.make_send();
    assert_eq!(*normal, 0);
}

/*
#[test]
fn simple_box_make_immortal() {
    use crate::FlexRcImplImmortal;

    let flex = <FlexRc<_, _> as FlexRcImpl<_>>::new(0);
    let normal = flex.make_immortal();
    assert_eq!(*normal, 0);
}
*/

#[test]
fn send_box() {
    let flex = <FlexRc<_, _> as FlexRcImplSend<_>>::new(Box::new(5));

    let other = flex.clone();
    let _thread = thread::spawn(move || {
        let _cloned = other.clone();
    });
}

#[test]
fn send_make_simple() {
    let immortal = <FlexRc<_, _> as FlexRcImplSend<_>>::new(0);
    let normal = immortal.make_simple();
    assert_eq!(*normal, 0);

    let _thread = thread::spawn(move || {
        let normal = immortal.make_simple();
        assert_eq!(*normal, 0);
    });
}

#[test]
fn send_make_immortal() {
    let immortal = <FlexRc<_, _> as FlexRcImplSend<_>>::new(0);
    let normal = immortal.make_immortal();
    assert_eq!(*normal, 0);

    let _thread = thread::spawn(move || {
        let normal = immortal.make_immortal();
        assert_eq!(*normal, 0);
    });
}

/*
#[test]
fn immortal_make_simple() {
    use crate::FlexRcImplImmortal;

    let immortal = <FlexRc<_, _> as FlexRcImplImmortal<_>>::new(0);
    let normal = immortal.make_simple();
    assert_eq!(*normal, 0);
}
*/

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
