use core::slice::Chunks;
use std::hash::Hash;
use std::{collections::HashSet, slice::Windows};

#[inline(always)]
fn intersection<T: Sized + Hash + Eq>(wx: Windows<T>, wy: Windows<T>) -> u64 {
    let len = wx.len();
    let hash: HashSet<&[T]> = wx.fold(HashSet::with_capacity(len), |mut acc, val| {
        acc.insert(val);
        acc
    });

    let mut len: u64 = 0;
    for w in wy {
        if hash.contains(w) {
            len += 2;
        }
    }

    len
}

#[inline(always)]
fn short_length<T: Sized + Hash + Eq>(wx: Windows<T>, wy: Windows<T>) -> f64 {
    let nx: usize = wx.len();
    let ny: usize = wy.len();
    let len = intersection(wx, wy);

    len as f64 / (nx as f64 + ny as f64)
}

#[inline(always)]
fn long_length<T: Sized + Hash + Eq>(cx: Chunks<T>, cy: &mut Chunks<T>) -> f64 {
    let mut len = 0;
    for chunk in cx {
        let wx: Windows<T> = chunk.windows(2);
        if let Some(ch) = cy.next() {
            let wy = ch.windows(2);
            len += intersection(wx, wy);
        }
    }

    len as f64
}

/**
    Calculates Sørensen–Dice coefficient
    https://en.wikipedia.org/wiki/Sørensen–Dice_coefficient

    Examples:

    ```ignore
    use crate::sorensen::distance;

    let string = "night";
    let string_to_compare = "nacht";
    let dst: f64 = distance(string.as_bytes(), string_to_compare.as_bytes()); // 0.25

    ```
**/
#[inline(always)]
pub fn distance<T: Sized + Hash + Eq>(x: &[T], y: &[T]) -> f64 {
    let x_len = x.len() - 1;
    let y_len = y.len() - 1;

    if x_len + y_len < 10000 {
        short_length(x.windows(2), y.windows(2))
    } else {
        let len = long_length(x.chunks(500), &mut y.chunks(500));
        len / (x_len as f64 + y_len as f64)
    }
}
