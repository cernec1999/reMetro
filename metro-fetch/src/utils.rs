/// Extension of Vec to insert an element while maintaining sorted order.
pub trait SortedVec<T> {
    /// Inserts an element into the vector, maintaining sorted order.
    fn insert_sorted(&mut self, value: T)
    where
        T: Ord;
}

impl<T> SortedVec<T> for Vec<T> {
    #[inline]
    fn insert_sorted(&mut self, value: T)
    where
        T: Ord,
    {
        let pos = match self.binary_search(&value) {
            Ok(pos) | Err(pos) => pos,
        };
        self.insert(pos, value);
    }
}