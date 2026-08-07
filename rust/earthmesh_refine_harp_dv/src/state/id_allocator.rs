use super::SiteId;

/// Hands out site ids, and never hands one out twice.
///
/// Monotonic on purpose. A site that is removed leaves its id behind rather
/// than freeing it, because the id is what a lineage record, a report row and a
/// checkpoint all refer to. Reusing it would make an old record point at a new
/// site, which is worse than running out of numbers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SiteIdAllocator {
    next: u64,
}

impl SiteIdAllocator {
    /// An allocator whose first id is `first`.
    ///
    /// A mesh arriving with sites already in it starts the counter past them,
    /// so an adapted site can never collide with an inherited one.
    pub fn starting_at(first: u64) -> Self {
        Self { next: first }
    }

    pub fn allocate(&mut self) -> SiteId {
        let id = SiteId(self.next);
        self.next += 1;
        id
    }

    /// The id the next call would return, without taking it.
    pub fn peek(&self) -> SiteId {
        SiteId(self.next)
    }

    pub fn issued(&self) -> u64 {
        self.next
    }
}
