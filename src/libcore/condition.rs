// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*! Condition handling */

use prelude::*;
use task;
use task::local_data::{local_data_pop, local_data_set};

// helper for transmutation, shown below.
type RustClosure = (int, int);

pub struct Handler<T, U> {
    handle: RustClosure,
    prev: Option<@Handler<T, U>>,
}

pub struct Condition<'self, T, U> {
    name: &'static str,
    key: task::local_data::LocalDataKey<'self, Handler<T, U>>
}

pub impl<'self, T, U> Condition<'self, T, U> {
    fn trap(&self, h: &'self fn(T) -> U) -> Trap<'self, T, U> {
        unsafe {
            let p : *RustClosure = ::cast::transmute(&h);
            let prev = task::local_data::local_data_get(self.key);
            let h = @Handler { handle: *p, prev: prev };
            Trap { cond: self, handler: h }
        }
    }

    fn raise(&self, t: T) -> U {
        let msg = fmt!("Unhandled condition: %s: %?", self.name, t);
        self.raise_default(t, || fail!(copy msg))
    }

    fn raise_default(&self, t: T, default: &fn() -> U) -> U {
        unsafe {
            match local_data_pop(self.key) {
                None => {
                    debug!("Condition.raise: found no handler");
                    default()
                }
                Some(handler) => {
                    debug!("Condition.raise: found handler");
                    match handler.prev {
                        None => {}
                        Some(hp) => local_data_set(self.key, hp)
                    }
                    let handle : &fn(T) -> U =
                        ::cast::transmute(handler.handle);
                    let u = handle(t);
                    local_data_set(self.key, handler);
                    u
                }
            }
        }
    }
}

struct Trap<'self, T, U> {
    cond: &'self Condition<'self, T, U>,
    handler: @Handler<T, U>
}

pub impl<'self, T, U> Trap<'self, T, U> {
    fn in<V>(&self, inner: &'self fn() -> V) -> V {
        unsafe {
            let _g = Guard { cond: self.cond };
            debug!("Trap: pushing handler to TLS");
            local_data_set(self.cond.key, self.handler);
            inner()
        }
    }
}

struct Guard<'self, T, U> {
    cond: &'self Condition<'self, T, U>
}

#[unsafe_destructor]
impl<'self, T, U> Drop for Guard<'self, T, U> {
    fn finalize(&self) {
        unsafe {
            debug!("Guard: popping handler from TLS");
            let curr = local_data_pop(self.cond.key);
            match curr {
                None => {}
                Some(h) => match h.prev {
                    None => {}
                    Some(hp) => local_data_set(self.cond.key, hp)
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    condition! {
        sadness: int -> int;
    }

    fn trouble(i: int) {
        debug!("trouble: raising condition");
        let j = sadness::cond.raise(i);
        debug!("trouble: handler recovered with %d", j);
    }

    fn nested_trap_test_inner() {
        let mut inner_trapped = false;

        do sadness::cond.trap(|_j| {
            debug!("nested_trap_test_inner: in handler");
            inner_trapped = true;
            0
        }).in {
            debug!("nested_trap_test_inner: in protected block");
            trouble(1);
        }

        assert!(inner_trapped);
    }

    #[test]
    fn nested_trap_test_outer() {
        let mut outer_trapped = false;

        do sadness::cond.trap(|_j| {
            debug!("nested_trap_test_outer: in handler");
            outer_trapped = true; 0
        }).in {
            debug!("nested_guard_test_outer: in protected block");
            nested_trap_test_inner();
            trouble(1);
        }

        assert!(outer_trapped);
    }

    fn nested_reraise_trap_test_inner() {
        let mut inner_trapped = false;

        do sadness::cond.trap(|_j| {
            debug!("nested_reraise_trap_test_inner: in handler");
            inner_trapped = true;
            let i = 10;
            debug!("nested_reraise_trap_test_inner: handler re-raising");
            sadness::cond.raise(i)
        }).in {
            debug!("nested_reraise_trap_test_inner: in protected block");
            trouble(1);
        }

        assert!(inner_trapped);
    }

    #[test]
    fn nested_reraise_trap_test_outer() {
        let mut outer_trapped = false;

        do sadness::cond.trap(|_j| {
            debug!("nested_reraise_trap_test_outer: in handler");
            outer_trapped = true; 0
        }).in {
            debug!("nested_reraise_trap_test_outer: in protected block");
            nested_reraise_trap_test_inner();
        }

        assert!(outer_trapped);
    }

    #[test]
    fn test_default() {
        let mut trapped = false;

        do sadness::cond.trap(|j| {
            debug!("test_default: in handler");
            sadness::cond.raise_default(j, || { trapped=true; 5 })
        }).in {
            debug!("test_default: in protected block");
            trouble(1);
        }

        assert!(trapped);
    }
}
