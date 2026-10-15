//! QJL projection S of TurboQuant_prod, i.i.d. N(0, 1), d×d row-major. Drawn from its own stream,
//! so it stays independent of the rotation even with the same seed.

use crate::rotation::Rng;
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static CACHE: RefCell<Option<(usize, u32, Rc<Vec<f32>>)>> = RefCell::new(None);
}

pub fn matrix(d: usize, seed: u32) -> Rc<Vec<f32>> {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((cd, cs, m)) = c.as_ref() {
            if *cd == d && *cs == seed {
                return m.clone();
            }
        }
        let mut s = vec![0.0f64; d * d];
        Rng(seed as u64 ^ 0x514a_4c00_0000_0000).gauss_fill(&mut s);
        let m = Rc::new(s.iter().map(|&x| x as f32).collect::<Vec<f32>>());
        *c = Some((d, seed, m.clone()));
        m
    })
}
