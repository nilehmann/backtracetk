use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

pub trait Partial {
    type Complete;

    fn merge_with(self, other: Self) -> Self;

    fn into_complete(self) -> Self::Complete;
}

impl<T> Partial for Option<T>
where
    T: Default,
{
    type Complete = T;

    fn merge_with(self, other: Self) -> Self {
        other.or(self)
    }

    fn into_complete(self) -> Self::Complete {
        self.unwrap_or_default()
    }
}

impl<T> Partial for Vec<T> {
    type Complete = Vec<T>;

    fn merge_with(mut self, other: Self) -> Self {
        self.extend(other);
        self
    }

    fn into_complete(self) -> Self::Complete {
        self
    }
}

impl<K, V, S> Partial for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    type Complete = HashMap<K, V, S>;

    fn merge_with(mut self, other: Self) -> Self {
        self.extend(other);
        self
    }

    fn into_complete(self) -> Self::Complete {
        self
    }
}

pub trait Complete {
    type Partial: Partial;

    fn into_partial(self) -> Self::Partial;
}

impl<T> Complete for Vec<T> {
    type Partial = Vec<T>;

    fn into_partial(self) -> Self::Partial {
        self
    }
}

impl<K, V, S> Complete for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    type Partial = HashMap<K, V, S>;

    fn into_partial(self) -> Self::Partial {
        self
    }
}

impl Complete for bool {
    type Partial = Option<bool>;

    fn into_partial(self) -> Self::Partial {
        Some(self)
    }
}

impl Complete for String {
    type Partial = Option<String>;

    fn into_partial(self) -> Self::Partial {
        Some(self)
    }
}
