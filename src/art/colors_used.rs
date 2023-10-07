use std::collections::HashSet;

use crate::color::ColorKey;

#[derive(Default, PartialEq)]
pub struct ColorsUsed {
    vector: Vec<ColorKey>,
    set: HashSet<ColorKey>,
}

impl std::fmt::Debug for ColorsUsed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.vector.fmt(f)
    }
}

impl ColorsUsed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, color: ColorKey) {
        if self.set.insert(color) {
            self.vector.push(color);
        }
    }

    pub fn as_slice(&self) -> &[ColorKey] {
        self.vector.as_slice()
    }

    pub fn iter(&self) -> <&'_ Self as IntoIterator>::IntoIter {
        self.into_iter()
    }
}

impl Extend<ColorKey> for ColorsUsed {
    fn extend<T: IntoIterator<Item = ColorKey>>(&mut self, iter: T) {
        for color in iter {
            self.insert(color);
        }
    }
}

impl<'a> IntoIterator for &'a ColorsUsed {
    type Item = ColorKey;
    type IntoIter = std::iter::Copied<std::slice::Iter<'a, ColorKey>>;
    fn into_iter(self) -> Self::IntoIter {
        self.vector.iter().copied()
    }
}
