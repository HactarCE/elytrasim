/// `len` evenly spaced points from `lo` to `hi`,
/// both endpoints included.
/// a single point is `lo == hi` with `len == 1`.
#[derive(Debug, Clone)]
pub struct Frange {
    lo: f64,
    hi: f64,
    len: usize,
}
impl Frange {
    pub fn from_singleton(x: f64) -> Self {
        Self {
            lo: x,
            hi: x,
            len: 1,
        }
    }

    pub fn from_n(lo: f64, hi: f64, len: usize) -> Self {
        assert!(lo < hi);
        assert!(len >= 2);
        Self { lo, hi, len }
    }

    /// stride is an upper bound on the returned stride.
    /// ie `ret.stride().unwrap() <= stride`.
    pub fn from_stride(lo: f64, hi: f64, stride: f64) -> Self {
        assert!(lo < hi);
        assert!(stride > 0.0);
        assert!(stride < hi - lo);
        let span = hi - lo;
        let mut intervals = (span / stride).ceil() as usize;
        // the division can round down onto an integer, leaving the actual
        // stride an ulp above the bound
        if span / intervals as f64 > stride {
            intervals += 1;
        }
        Self::from_n(lo, hi, intervals + 1)
    }

    /// `None` for a singleton.
    pub fn stride(&self) -> Option<f64> {
        if self.len == 1 {
            return None;
        }
        Some((self.hi - self.lo) / (self.len - 1) as f64)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// the `i`th point; exact at both endpoints.
    pub fn get(&self, i: usize) -> f64 {
        assert!(i < self.len);
        if i == self.len - 1 {
            self.hi
        } else {
            self.lo + (self.hi - self.lo) * (i as f64 / (self.len - 1) as f64)
        }
    }

    pub fn iter(&self) -> FRangeIter {
        FRangeIter {
            range: self.clone(),
            front: 0,
            back: self.len,
        }
    }
}

impl IntoIterator for Frange {
    type Item = f64;
    type IntoIter = FRangeIter;
    fn into_iter(self) -> FRangeIter {
        let back = self.len;
        FRangeIter {
            range: self,
            front: 0,
            back,
        }
    }
}

impl IntoIterator for &Frange {
    type Item = f64;
    type IntoIter = FRangeIter;
    fn into_iter(self) -> FRangeIter {
        self.iter()
    }
}

#[derive(Debug, Clone)]
pub struct FRangeIter {
    range: Frange,
    /// next index from the front
    front: usize,
    /// one past the next index from the back
    back: usize,
}

impl Iterator for FRangeIter {
    type Item = f64;

    fn next(&mut self) -> Option<f64> {
        if self.front == self.back {
            return None;
        }
        let x = self.range.get(self.front);
        self.front += 1;
        Some(x)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.back - self.front;
        (n, Some(n))
    }

    fn nth(&mut self, n: usize) -> Option<f64> {
        self.front = self.front.saturating_add(n).min(self.back);
        self.next()
    }
}

impl DoubleEndedIterator for FRangeIter {
    fn next_back(&mut self) -> Option<f64> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        Some(self.range.get(self.back))
    }
}

impl ExactSizeIterator for FRangeIter {}

impl std::iter::FusedIterator for FRangeIter {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_exact() {
        let r = Frange::from_n(0.1, 0.7, 7);
        let xs: Vec<f64> = r.iter().collect();
        assert_eq!(xs.len(), 7);
        assert_eq!(xs[0], 0.1);
        assert_eq!(xs[6], 0.7);
        assert!(xs.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn stride_is_upper_bound() {
        for &(lo, hi, s) in &[
            (0.0, 1.0, 0.1),
            (0.0, 1.0, 0.3),
            (-90.0, 90.0, 0.5),
            (0.0, 3.0, 0.7),
        ] {
            let r = Frange::from_stride(lo, hi, s);
            let rs = r.stride().unwrap();
            assert!(rs <= s, "{lo} {hi} {s}: {rs}");
            // and not needlessly fine: one fewer point would exceed it
            assert!((hi - lo) / (r.len() - 2) as f64 > s);
        }
    }

    #[test]
    fn single_point() {
        let r = Frange::from_singleton(2.5);
        assert_eq!(r.iter().collect::<Vec<_>>(), vec![2.5]);
        assert_eq!(r.stride(), None);
    }

    #[test]
    #[should_panic]
    fn from_n_rejects_single_point() {
        Frange::from_n(2.5, 2.5, 1);
    }

    #[test]
    fn double_ended() {
        let r = Frange::from_n(0.0, 4.0, 5);
        let mut it = r.iter();
        assert_eq!(it.next(), Some(0.0));
        assert_eq!(it.next_back(), Some(4.0));
        assert_eq!(it.len(), 3);
        assert_eq!(it.rev().collect::<Vec<_>>(), vec![3.0, 2.0, 1.0]);
        assert_eq!(r.iter().nth(10), None);
    }
}
