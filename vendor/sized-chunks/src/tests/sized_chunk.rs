#![allow(clippy::unit_arg)]

use std::fmt::Debug;
use std::iter::FromIterator;
use std::panic::{catch_unwind, AssertUnwindSafe};

use proptest::{arbitrary::any, collection::vec, prelude::*, proptest};
use proptest_derive::Arbitrary;

use crate::sized_chunk::Chunk;

#[test]
fn validity_invariant() {
    assert!(Some(Chunk::<Box<()>>::new()).is_some());
}

#[derive(Debug)]
struct InputVec<A>(Vec<A>);

impl<A> InputVec<A> {
    fn unwrap(self) -> Vec<A> {
        self.0
    }
}

impl<A> Arbitrary for InputVec<A>
where
    A: Arbitrary + Debug,
    <A as Arbitrary>::Strategy: 'static,
{
    type Parameters = usize;
    type Strategy = BoxedStrategy<InputVec<A>>;
    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        #[allow(clippy::redundant_closure)]
        proptest::collection::vec(any::<A>(), 0..Chunk::<u32>::CAPACITY)
            .prop_map(|v| InputVec(v))
            .boxed()
    }
}

#[derive(Arbitrary, Debug)]
enum Construct<A>
where
    A: Arbitrary,
    <A as Arbitrary>::Strategy: 'static,
{
    Empty,
    Single(A),
    Pair((A, A)),
    DrainFrom(InputVec<A>),
    CollectFrom(InputVec<A>, usize),
    FromFront(InputVec<A>, usize),
    FromBack(InputVec<A>, usize),
}

#[derive(Arbitrary, Debug)]
enum Action<A>
where
    A: Arbitrary,
    <A as Arbitrary>::Strategy: 'static,
{
    PushFront(A),
    PushBack(A),
    PopFront,
    PopBack,
    DropLeft(usize),
    DropRight(usize),
    SplitOff(usize),
    Append(Construct<A>),
    DrainFromFront(Construct<A>, usize),
    DrainFromBack(Construct<A>, usize),
    Set(usize, A),
    Insert(usize, A),
    InsertFrom(Vec<A>, usize),
    Remove(usize),
    Drain,
    Clear,
}

impl<A> Construct<A>
where
    A: Arbitrary + Clone + Debug + Eq,
    <A as Arbitrary>::Strategy: 'static,
{
    fn make(self) -> Chunk<A> {
        match self {
            Construct::Empty => {
                let out = Chunk::new();
                assert!(out.is_empty());
                out
            }
            Construct::Single(value) => {
                let out = Chunk::unit(value.clone());
                assert_eq!(out, vec![value]);
                out
            }
            Construct::Pair((left, right)) => {
                let out = Chunk::pair(left.clone(), right.clone());
                assert_eq!(out, vec![left, right]);
                out
            }
            Construct::DrainFrom(vec) => {
                let vec = vec.unwrap();
                let mut source = Chunk::from_iter(vec.iter().cloned());
                let out = Chunk::drain_from(&mut source);
                assert!(source.is_empty());
                assert_eq!(out, vec);
                out
            }
            Construct::CollectFrom(vec, len) => {
                let mut vec = vec.unwrap();
                if vec.is_empty() {
                    return Chunk::new();
                }
                let len = len % vec.len();
                let mut source = vec.clone().into_iter();
                let out = Chunk::collect_from(&mut source, len);
                let expected_remainder = vec.split_off(len);
                let remainder: Vec<_> = source.collect();
                assert_eq!(expected_remainder, remainder);
                assert_eq!(out, vec);
                out
            }
            Construct::FromFront(vec, len) => {
                let mut vec = vec.unwrap();
                if vec.is_empty() {
                    return Chunk::new();
                }
                let len = len % vec.len();
                let mut source = Chunk::from_iter(vec.iter().cloned());
                let out = Chunk::from_front(&mut source, len);
                let remainder = vec.split_off(len);
                assert_eq!(source, remainder);
                assert_eq!(out, vec);
                out
            }
            Construct::FromBack(vec, len) => {
                let mut vec = vec.unwrap();
                if vec.is_empty() {
                    return Chunk::new();
                }
                let len = len % vec.len();
                let mut source = Chunk::from_iter(vec.iter().cloned());
                let out = Chunk::from_back(&mut source, len);
                let remainder = vec.split_off(vec.len() - len);
                assert_eq!(out, remainder);
                assert_eq!(source, vec);
                out
            }
        }
    }
}

fn assert_panic<A, F>(f: F)
where
    F: FnOnce() -> A,
{
    let result = catch_unwind(AssertUnwindSafe(f));
    assert!(
        result.is_err(),
        "action that should have panicked didn't panic"
    );
}

proptest! {
    #[test]
    fn test_constructors(cons: Construct<u32>) {
        cons.make();
    }

    #[test]
    fn test_actions(cons: Construct<u32>, actions in vec(any::<Action<u32>>(), 0..super::action_count())) {
    let capacity = Chunk::<u32>::CAPACITY;
    let mut chunk = cons.make();
    let mut guide: Vec<_> = chunk.iter().cloned().collect();
    for action in actions {
        match action {
            Action::PushFront(value) => {
                if chunk.is_full() {
                    assert_panic(|| chunk.push_front(value));
                } else {
                    chunk.push_front(value);
                    guide.insert(0, value);
                }
            }
            Action::PushBack(value) => {
                if chunk.is_full() {
                    assert_panic(|| chunk.push_back(value));
                } else {
                    chunk.push_back(value);
                    guide.push(value);
                }
            }
            Action::PopFront => {
                if chunk.is_empty() {
                    assert_panic(|| chunk.pop_front());
                } else {
                    assert_eq!(chunk.pop_front(), guide.remove(0));
                }
            }
            Action::PopBack => {
                if chunk.is_empty() {
                    assert_panic(|| chunk.pop_back());
                } else {
                    assert_eq!(chunk.pop_back(), guide.pop().unwrap());
                }
            }
            Action::DropLeft(index) => {
                if index > chunk.len() {
                    assert_panic(|| chunk.drop_left(index));
                } else {
                    chunk.drop_left(index);
                    guide.drain(..index);
                }
            }
            Action::DropRight(index) => {
                if index > chunk.len() {
                    assert_panic(|| chunk.drop_right(index));
                } else {
                    chunk.drop_right(index);
                    guide.drain(index..);
                }
            }
            Action::SplitOff(index) => {
                if index > chunk.len() {
                    assert_panic(|| chunk.split_off(index));
                } else {
                    let chunk_off = chunk.split_off(index);
                    let guide_off = guide.split_off(index);
                    assert_eq!(chunk_off, guide_off);
                }
            }
            Action::Append(other) => {
                let mut other = other.make();
                let mut other_guide: Vec<_> = other.iter().cloned().collect();
                if other.len() + chunk.len() > capacity {
                    assert_panic(|| chunk.append(&mut other));
                } else {
                    chunk.append(&mut other);
                    guide.append(&mut other_guide);
                }
            }
            Action::DrainFromFront(other, count) => {
                let mut other = other.make();
                let mut other_guide: Vec<_> = other.iter().cloned().collect();
                if count > other.len() || chunk.len() + count > capacity {
                    assert_panic(|| chunk.drain_from_front(&mut other, count));
                } else {
                    chunk.drain_from_front(&mut other, count);
                    guide.extend(other_guide.drain(..count));
                    assert_eq!(other, other_guide);
                }
            }
            Action::DrainFromBack(other, count) => {
                let mut other = other.make();
                let mut other_guide: Vec<_> = other.iter().cloned().collect();
                if count > other.len() || chunk.len() + count > capacity {
                    assert_panic(|| chunk.drain_from_back(&mut other, count));
                } else {
                    let other_index = other.len() - count;
                    chunk.drain_from_back(&mut other, count);
                    guide = other_guide
                        .drain(other_index..)
                        .chain(guide.into_iter())
                        .collect();
                    assert_eq!(other, other_guide);
                }
            }
            Action::Set(index, value) => {
                if index >= chunk.len() {
                    assert_panic(|| chunk.set(index, value));
                } else {
                    chunk.set(index, value);
                    guide[index] = value;
                }
            }
            Action::Insert(index, value) => {
                if index > chunk.len() || chunk.is_full() {
                    assert_panic(|| chunk.insert(index, value));
                } else {
                    chunk.insert(index, value);
                    guide.insert(index, value);
                }
            }
            Action::InsertFrom(values, index) => {
                if index > chunk.len() || chunk.len() + values.len() > capacity {
                    assert_panic(|| chunk.insert_from(index, values));
                } else {
                    chunk.insert_from(index, values.clone());
                    for value in values.into_iter().rev() {
                        guide.insert(index, value);
                    }
                }
            }
            Action::Remove(index) => {
                if index >= chunk.len() {
                    assert_panic(|| chunk.remove(index));
                } else {
                    assert_eq!(chunk.remove(index), guide.remove(index));
                }
            }
            Action::Drain => {
                let drained: Vec<_> = chunk.drain().collect();
                let drained_guide: Vec<_> = guide.drain(..).collect();
                assert_eq!(drained, drained_guide);
            }
            Action::Clear => {
                chunk.clear();
                guide.clear();
            }
        }
        assert_eq!(chunk, guide);
        assert!(guide.len() <= capacity);
    }
    }
}

#[cfg(feature = "refpool")]
mod refpool_test {
    use super::*;
    use refpool::{Pool, PoolRef};

    #[test]
    fn stress_test() {
        let pool_size = 1024;
        let allocs = 2048;

        let pool: Pool<Chunk<usize>> = Pool::new(pool_size);
        pool.fill();

        for _ in 0..8 {
            let mut store = Vec::new();
            for _ in 0..allocs {
                store.push(PoolRef::default(&pool));
            }
            for chunk in &mut store {
                let chunk = PoolRef::make_mut(&pool, chunk);
                for _ in 0..32 {
                    chunk.push_front(1);
                    chunk.push_back(2);
                }
            }
            let mut expected: Chunk<usize> = Chunk::new();
            for _ in 0..32 {
                expected.push_back(2);
                expected.push_front(1);
            }
            for chunk in &store {
                assert_eq!(expected, **chunk);
            }
        }
    }
}
