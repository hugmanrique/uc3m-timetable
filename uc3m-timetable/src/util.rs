use std::cell::RefCell;
use std::rc::Rc;

pub type ProcessResult<E> = Rc<RefCell<Result<(), E>>>;

/// An [`Iterator`] adapter that yields `Some(item)` elements while
/// the wrapped iterator returns `Some(Ok(item))`. If the iterator
/// returns `Some(Err(_))`, the error is stored and can be accessed
/// by borrowing [`Self::state`].
pub struct Process<I, E> {
    iter: I,
    state: ProcessResult<E>,
}

impl<T, E, I> Iterator for Process<I, E>
where
    I: Iterator<Item = Result<T, E>>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.state.borrow().is_err() {
            return None;
        }
        match self.iter.next() {
            Some(Ok(item)) => Some(item),
            Some(Err(e)) => {
                *self.state.borrow_mut() = Err(e);
                None
            }
            None => None,
        }
    }
}

/// Returns an [`Iterator`] that yields `Some(item)` elements while
/// the wrapped iterator returns `Some(Ok(item))`. If the iterator
/// returns `Some(Err(err))`, the error is stored and can be accessed
/// by borrowing the mutable `Result`. Otherwise the result is `Ok(())`.
#[must_use]
pub fn process<T, E, I>(iter: I) -> (Process<I::IntoIter, E>, ProcessResult<E>)
where
    I: IntoIterator<Item = Result<T, E>>,
{
    let iter = iter.into_iter();
    let state = Rc::new(RefCell::new(Ok(())));
    let adapter = Process {
        iter,
        state: state.clone(),
    };
    (adapter, state)
}

#[cfg(test)]
mod tests {
    use crate::util::process;

    #[derive(Debug, Eq, PartialEq, Copy, Clone)]
    struct DummyError {}

    #[test]
    fn all_ok() {
        let vec: Vec<Result<u8, DummyError>> = vec![Ok(1), Ok(2), Ok(3)];
        let (mut iter, result) = process(vec);
        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.next(), Some(2));
        assert_eq!(iter.next(), Some(3));
        assert_eq!(iter.next(), None);
        assert_eq!(*result.borrow(), Ok(()));
    }

    #[test]
    fn one_err() {
        let (mut iter, result) = process(vec![Ok(true), Err(DummyError {}), Ok(false)]);
        assert_eq!(iter.next(), Some(true));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(*result.borrow(), Err(DummyError {}));
    }

    #[test]
    fn chaining() {
        let (iter, result) = process(vec![Ok(10), Ok(20), Err(DummyError {})]);
        let mut iter = iter.map(|x| x + 1);
        assert_eq!(iter.next(), Some(11));
        assert_eq!(iter.next(), Some(21));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(*result.borrow(), Err(DummyError {}));
    }
}
