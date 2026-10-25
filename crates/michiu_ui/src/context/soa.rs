use crate::{DebugStore, Entity, MichiuError, OptionTraceExt, Result};

pub trait MichiuSoA {
    type Item;

    fn find(&self, id: Entity) -> Option<&Self::Item>;

    fn find_mut(&mut self, id: Entity) -> Option<&mut Self::Item>;

    /// # Errors
    ///
    /// Returns [`MichiuError::ComponentNotFound`] If `Entity` is not found.
    #[inline]
    fn require(&self, id: Entity) -> Result<&Self::Item> {
        self.find(id).ok_or_else(|| MichiuError::ComponentNotFound {
            id,
            component: std::any::type_name::<Self>(),
        })
    }

    /// # Errors
    ///
    /// Returns [`MichiuError::ComponentNotFound`] If `Entity` is not found.
    #[inline]
    fn require_mut(&mut self, id: Entity) -> Result<&mut Self::Item> {
        let component = std::any::type_name::<Self>();
        self.find_mut(id)
            .ok_or(MichiuError::ComponentNotFound { id, component })
    }

    /// Panic-Prone Immutable Reference Accessors
    #[inline]
    #[track_caller]
    #[expect(clippy::panic)]
    fn at(&self, id: Entity) -> &Self::Item {
        self.find(id).unwrap_or_else(|| {
            let type_name = std::any::type_name::<Self>();
            let caller = std::panic::Location::caller();

            panic!(
                "[Michiu UI] SoA Component Access Failed\n\
                    Caller Loc    : {caller}\n\
                    Target Entity : {id:?}\n\
                    SoA Container : {type_name}\n\
                    Possible causes :\n\
                        - The entity was already despawned (dangling Entity).\n\
                        - The component was not attached to this entity during spawn.\n\
                        - The component was removed before this access.\n\
                "
            )
        })
    }

    #[inline]
    #[track_caller]
    #[expect(clippy::panic)]
    fn at_mut(&mut self, id: Entity) -> &mut Self::Item {
        self.find_mut(id).unwrap_or_else(|| {
            let type_name = std::any::type_name::<Self>();
            let caller = std::panic::Location::caller();

            panic!(
                "[Michiu UI] SoA Component Mutable Access Failed\n\
                    Caller Loc    : {caller}\n\
                    Target Entity : {id:?}\n\
                    SoA Container : {type_name}\n\
                    Possible causes :\n\
                        - The entity was already despawned (dangling Entity).\n\
                        - The component was not attached to this entity during spawn.\n\
                        - The component was removed before this access.\n\
                "
            )
        })
    }

    /// Fallback to the default value
    #[inline]
    #[track_caller]
    fn find_or_default(&self, id: Entity, debug: &mut DebugStore) -> Self::Item
    where
        Self::Item: Default + Clone + std::fmt::Debug + Send + Sync + 'static,
    {
        self.find(id)
            .cloned()
            .unwrap_or_default_trace(Some(id), debug)
    }

    /// Fallback to the specified value
    #[inline]
    #[track_caller]
    fn find_or<'a>(
        &'a self,
        id: Entity,
        fallback: &'a Self::Item,
        debug: &mut DebugStore,
    ) -> &'a Self::Item
    where
        Self::Item: Default + Clone + std::fmt::Debug + Send + Sync + 'static,
    {
        #[cfg(not(feature = "trace-entity"))]
        {
            let _ = debug;
            self.find(id).unwrap_or(fallback)
        }

        #[cfg(feature = "trace-entity")]
        if let Some(val) = self.find(id) {
            val
        } else {
            use crate::{MichiuInfo, MichiuTrace, trace_entity};
            use std::sync::Arc;

            trace_entity!(Some(id), debug, || MichiuTrace::EntityInfo {
                detail: MichiuInfo::ValueNotFound,
                fallback: Some(Arc::new(fallback.clone())),
                add: Some(std::any::type_name::<Self::Item>()),
            });
            fallback
        }
    }

    #[inline]
    fn contains(&self, id: Entity) -> bool {
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
        $vis struct $name(pub(crate) slotmap::SecondaryMap<$crate::Entity, $item>);

        impl $crate::MichiuSoA for $name {
            type Item = $item;
            #[inline]
            fn find(&self, id: $crate::Entity) -> Option<&Self::Item> { self.0.get(id) }
            #[inline]
            fn find_mut(&mut self, id: $crate::Entity) -> Option<&mut Self::Item> { self.0.get_mut(id) }
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
        $vis struct $name(pub(crate) slotmap::SparseSecondaryMap<$crate::Entity, $item>);

        impl $crate::MichiuSoA for $name {
            type Item = $item;
            #[inline]
            fn find(&self, id: $crate::Entity) -> Option<&Self::Item> { self.0.get(id) }
            #[inline]
            fn find_mut(&mut self, id: $crate::Entity) -> Option<&mut Self::Item> { self.0.get_mut(id) }
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

#[cfg(test)]
#[expect(clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use slotmap::{Key, SlotMap};

    define_secondary!(struct TestSecondary(i32));
    define_sparse_secondary!(struct TestSparseSecondary(i32));
    define_slotmap!(struct TestSlotMap(Entity, u32));
    define_vec!(struct TestVec(i32));
    define_smallvec!(struct TestSmallVec(i32, 4));

    fn setup_entity() -> (SlotMap<Entity, ()>, Entity) {
        let mut sm = SlotMap::<Entity, ()>::with_key();
        let entity = sm.insert(());
        (sm, entity)
    }

    #[test]
    fn test_soa_find_and_contains() {
        let (_sm, entity) = setup_entity();
        let missing = Entity::null();

        let mut map = TestSecondary::default();
        map.insert(entity, 42);

        assert_eq!(map.find(entity), Some(&42));
        assert!(map.contains(entity));

        assert_eq!(map.find(missing), None);
        assert!(!map.contains(missing));
    }

    #[test]
    fn test_soa_find_mut() {
        let (_sm, entity) = setup_entity();
        let missing = Entity::null();

        let mut map = TestSecondary::default();
        map.insert(entity, 10);

        if let Some(val) = map.find_mut(entity) {
            *val += 5;
        }
        assert_eq!(map.find(entity), Some(&15));
        assert!(map.find_mut(missing).is_none());
    }

    #[test]
    fn test_soa_require_and_require_mut() {
        let (_sm, entity) = setup_entity();
        let missing = Entity::null();

        let mut map = TestSecondary::default();
        map.insert(entity, 100);

        // require
        assert_eq!(map.require(entity).unwrap(), &100);
        match map.require(missing) {
            Err(MichiuError::ComponentNotFound { id, component }) => {
                assert_eq!(id, missing);
                assert!(component.contains("TestSecondary"));
            }
            _ => panic!("Expected ComponentNotFound error"),
        }

        // require_mut
        *map.require_mut(entity).unwrap() = 200;
        assert_eq!(map.require(entity).unwrap(), &200);
        assert!(matches!(
            map.require_mut(missing),
            Err(MichiuError::ComponentNotFound { .. })
        ));
    }

    #[test]
    fn test_soa_at_and_at_mut() {
        let (_sm, entity) = setup_entity();
        let missing = Entity::null();

        let mut map = TestSecondary::default();
        map.insert(entity, 50);

        assert_eq!(*map.at(entity), 50);
        *map.at_mut(entity) = 60;
        assert_eq!(*map.at(entity), 60);

        // at の panic 検証
        let res_at = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = map.at(missing);
        }));
        assert!(res_at.is_err());

        // at_mut の panic 検証
        let res_at_mut = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = map.at_mut(missing);
        }));
        assert!(res_at_mut.is_err());
    }

    #[test]
    fn test_soa_find_or_default_and_find_or() {
        let (_sm, entity) = setup_entity();
        let missing = Entity::null();
        let mut debug = DebugStore::new();

        let mut map = TestSecondary::default();
        map.insert(entity, 77);

        // find_or_default
        assert_eq!(map.find_or_default(entity, &mut debug), 77);
        assert_eq!(map.find_or_default(missing, &mut debug), 0);

        // find_or
        let fallback = 999;
        assert_eq!(map.find_or(entity, &fallback, &mut debug), &77);
        assert_eq!(map.find_or(missing, &fallback, &mut debug), &999);
    }

    #[test]
    fn test_define_macros() {
        let (_sm, entity) = setup_entity();

        // define_sparse_secondary
        let mut sparse = TestSparseSecondary::default();
        sparse.insert(entity, 12);
        assert_eq!(sparse.find(entity), Some(&12));

        // define_slotmap
        let mut slot = TestSlotMap::default();
        let k = slot.insert(100);
        assert_eq!(slot[k], 100);

        // define_vec (Deref, DerefMut, IntoIterator)
        let mut v = TestVec::default();
        v.push(1);
        v.push(2);
        assert_eq!(v.len(), 2);
        assert_eq!((&v).into_iter().copied().collect::<Vec<_>>(), vec![1, 2]);

        // define_smallvec
        let mut sv = TestSmallVec::default();
        sv.push(10);
        assert_eq!(sv.len(), 1);
        assert_eq!(sv.into_iter().collect::<Vec<_>>(), vec![10]);
    }
}
