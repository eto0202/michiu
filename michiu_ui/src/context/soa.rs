use crate::{DebugStore, EntityId, MichiuError, Result};

pub trait MichiuSoA {
    type Item;

    /// 存在しない場合は None を返す
    fn find(&self, id: EntityId) -> Option<&Self::Item>;

    /// 可変参照用
    fn find_mut(&mut self, id: EntityId) -> Option<&mut Self::Item>;

    /// 存在しない場合は Err を返す
    #[inline]
    fn require(&self, id: EntityId) -> Result<&Self::Item> {
        self.find(id).ok_or_else(|| MichiuError::ComponentNotFound {
            id,
            component: std::any::type_name::<Self>(),
        })
    }

    /// 可変参照用
    #[inline]
    fn require_mut(&mut self, id: EntityId) -> Result<&mut Self::Item> {
        let component = std::any::type_name::<Self>();
        self.find_mut(id)
            .ok_or(MichiuError::ComponentNotFound { id, component })
    }

    /// パニック可能な不変参照アクセサ
    #[inline]
    #[track_caller]
    #[allow(clippy::panic)]
    fn at(&self, id: EntityId) -> &Self::Item {
        self.find(id).unwrap_or_else(|| {
            let type_name = std::any::type_name::<Self>();
            let caller = std::panic::Location::caller();

            panic!(
                "[Michiu UI] SoA Component Access Failed\n\
                    Caller Loc    : {caller}\n\
                    Target Entity : {id:?}\n\
                    SoA Container : {type_name}\n\
                    Possible causes :\n\
                        - The entity was already despawned (dangling EntityId).\n\
                        - The component was not attached to this entity during spawn.\n\
                        - The component was removed before this access.\n\
                "
            )
        })
    }

    /// パニック可能な可変参照アクセサ
    #[inline]
    #[track_caller]
    #[allow(clippy::panic)]
    fn at_mut(&mut self, id: EntityId) -> &mut Self::Item {
        self.find_mut(id).unwrap_or_else(|| {
            let type_name = std::any::type_name::<Self>();
            let caller = std::panic::Location::caller();

            panic!(
                "[Michiu UI] SoA Component Mutable Access Failed\n\
                    Caller Loc    : {caller}\n\
                    Target Entity : {id:?}\n\
                    SoA Container : {type_name}\n\
                    Possible causes :\n\
                        - The entity was already despawned (dangling EntityId).\n\
                        - The component was not attached to this entity during spawn.\n\
                        - The component was removed before this access.\n\
                "
            )
        })
    }

    /// デフォルト値でフォールバック
    #[inline]
    #[track_caller]
    fn find_or_default(&self, id: EntityId, debug: &mut DebugStore) -> Self::Item
    where
        Self::Item: Default + Clone + std::fmt::Debug + Send + Sync + 'static,
    {
        #[cfg(not(feature = "trace-lifecycle"))]
        {
            let _ = debug;
            self.find(id).cloned().unwrap_or_default()
        }

        #[cfg(feature = "trace-lifecycle")]
        if let Some(val) = self.find(id) {
            val.clone()
        } else {
            use crate::{MichiuInfo, MichiuTrace, trace_lifecycle};
            use std::sync::Arc;

            trace_lifecycle!(Some(id), debug, || MichiuTrace::Info {
                detail: MichiuInfo::ValueNotFound,
                fallback: Some(Arc::new(Self::Item::default())),
                add: Some(std::any::type_name::<Self::Item>()),
            });
            Self::Item::default()
        }
    }

    /// 指定の値でフォールバック
    #[inline]
    #[track_caller]
    fn find_or<'a>(
        &'a self,
        id: EntityId,
        fallback: &'a Self::Item,
        debug: &mut DebugStore,
    ) -> &'a Self::Item
    where
        Self::Item: Default + Clone + std::fmt::Debug + Send + Sync + 'static,
    {
        #[cfg(not(feature = "trace-lifecycle"))]
        {
            let _ = debug;
            self.find(id).unwrap_or(fallback)
        }

        #[cfg(feature = "trace-lifecycle")]
        if let Some(val) = self.find(id) {
            val
        } else {
            use crate::{MichiuInfo, MichiuTrace, trace_lifecycle};
            use std::sync::Arc;

            trace_lifecycle!(Some(id), debug, || MichiuTrace::Info {
                detail: MichiuInfo::ValueNotFound,
                fallback: Some(Arc::new(fallback.clone())),
                add: Some(std::any::type_name::<Self::Item>()),
            });
            fallback
        }
    }

    /// その要素がコンポーネントを保持しているか
    #[inline]
    fn contains(&self, id: EntityId) -> bool {
        self.find(id).is_some()
    }
}

#[macro_export]
macro_rules! define_slotmap {
    ($(#[$meta:meta])* $vis:vis struct $name:ident($key:ty, $item:ty) $(;)?) => {
        $(#[$meta])*
        #[derive(Debug, Default, derive_more::Deref, derive_more::DerefMut)]
        $vis struct $name(pub(crate) slotmap::SlotMap<$key, $item>);
    };
}

#[macro_export]
macro_rules! define_secondary {
    ($(#[$meta:meta])* $vis:vis struct $name:ident($item:ty) $(;)?) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone, derive_more::Deref, derive_more::DerefMut)]
        $vis struct $name(pub(crate) slotmap::SecondaryMap<$crate::EntityId, $item>);

        impl $crate::MichiuSoA for $name {
            type Item = $item;
            #[inline]
            fn find(&self, id: $crate::EntityId) -> Option<&Self::Item> { self.0.get(id) }
            #[inline]
            fn find_mut(&mut self, id: $crate::EntityId) -> Option<&mut Self::Item> { self.0.get_mut(id) }
        }
    };

    ($(#[$meta:meta])* $vis:vis struct $name:ident($key:ty, $item:ty) $(;)?) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone, derive_more::Deref, derive_more::DerefMut)]
        $vis struct $name(pub(crate) slotmap::SecondaryMap<$key, $item>);
    };
}

#[macro_export]
macro_rules! define_sparse_secondary {
    ($(#[$meta:meta])* $vis:vis struct $name:ident($item:ty) $(;)?) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone, derive_more::Deref, derive_more::DerefMut)]
        $vis struct $name(pub(crate) slotmap::SparseSecondaryMap<$crate::EntityId, $item>);

        impl $crate::MichiuSoA for $name {
            type Item = $item;
            #[inline]
            fn find(&self, id: $crate::EntityId) -> Option<&Self::Item> { self.0.get(id) }
            #[inline]
            fn find_mut(&mut self, id: $crate::EntityId) -> Option<&mut Self::Item> { self.0.get_mut(id) }
        }
    };

    ($(#[$meta:meta])* $vis:vis struct $name:ident($key:ty, $item:ty) $(;)?) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone, derive_more::Deref, derive_more::DerefMut)]
        $vis struct $name(pub(crate) slotmap::SparseSecondaryMap<$key, $item>);
    };
}

#[macro_export]
macro_rules! define_vec {
    ($(#[$meta:meta])* $vis:vis struct $name:ident($item:ty) $(;)?) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default, derive_more::Deref, derive_more::DerefMut, derive_more::IntoIterator)]
        #[into_iterator(owned, ref, ref_mut)]
        $vis struct $name(pub(crate) Vec<$item>);
    };
}

// SmallVec は 参照に対する IntoIterator を持っていない
#[macro_export]
macro_rules! define_smallvec {
    ($(#[$meta:meta])* $vis:vis struct $name:ident($item:ty, $size:expr) $(;)?) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default, derive_more::Deref, derive_more::DerefMut, derive_more::IntoIterator)]
        $vis struct $name(pub(crate) smallvec::SmallVec<[$item; $size]>);
    };
}
