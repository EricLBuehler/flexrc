#![cfg(test)]

use crate::{FlexRc, FlexRcImpl};

#[test]
fn unsync_box() {
    let flex = FlexRc::new(Box::new(5));
    let other = flex.clone();
}
