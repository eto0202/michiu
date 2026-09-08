use crate::{EntityId, MichiuError};

pub(crate) trait MichiuSoA {
    type Item;

    /// 存在しない場合は None を返す
    fn get(&self, id: EntityId) -> Option<&Self::Item>;

    /// 可変参照用
    fn get_mut(&mut self, id: EntityId) -> Option<&mut Self::Item>;

    /// 存在しない場合は Err を返す
    #[inline]
    fn require(&self, id: EntityId) -> Result<&Self::Item, MichiuError> {
        self.get(id).ok_or_else(|| MichiuError::ComponentNotFound {
            id,
            component: std::any::type_name::<Self>(),
        })
    }

    /// 可変参照用
    #[inline]
    fn require_mut(&mut self, id: EntityId) -> Result<&mut Self::Item, MichiuError> {
        let component = std::any::type_name::<Self>();
        self.get_mut(id)
            .ok_or(MichiuError::ComponentNotFound { id, component })
    }

    /// パニック可能な不変参照アクセサ
    #[inline]
    #[allow(clippy::panic)]
    fn at(&self, id: EntityId) -> &Self::Item {
        let type_name = std::any::type_name::<Self>();
        self.get(id).unwrap_or_else(|| {
            panic!(
                "[Michiu UI] SoA Component Access Failed\n\
                    Target Entity : {id:?}\n\
                    SoA Container : {type_name}\n\
                    Possible causes:\n\
                        - The entity was already despawned (dangling EntityId).\n\
                        - The component was not attached to this entity during spawn.\n\
                        - The component was removed before this access.\n\
                "
            )
        })
    }

    /// パニック可能な可変参照アクセサ
    #[inline]
    #[allow(clippy::panic)]
    fn at_mut(&mut self, id: EntityId) -> &mut Self::Item {
        let type_name = std::any::type_name::<Self>();
        self.get_mut(id).unwrap_or_else(|| {
            panic!(
                "[Michiu UI] SoA Component Mutable Access Failed\n\
                    Target Entity : {id:?}\n\
                    SoA Container : {type_name}\n\
                    Possible causes:\n\
                        - The entity was already despawned (dangling EntityId).\n\
                        - The component was not attached to this entity during spawn.\n\
                        - The component was removed before this access.\n\
                "
            )
        })
    }

    /// デフォルト値でフォールバック
    #[inline]
    fn get_or_default(&self, id: EntityId) -> Self::Item
    where
        Self::Item: Default + Clone,
    {
        self.get(id).cloned().unwrap_or_default()
    }

    /// 指定の値でフォールバック
    #[inline]
    fn get_or<'a>(&'a self, id: EntityId, fallback: &'a Self::Item) -> &'a Self::Item {
        self.get(id).unwrap_or(fallback)
    }

    /// その要素がコンポーネントを保持しているか
    #[inline]
    fn contains(&self, id: EntityId) -> bool {
        self.get(id).is_some()
    }
}

#[macro_export]
macro_rules! define_slotmap {
    ($(#[$meta:meta])* $vis:vis struct $name:ident($key:ty => $item:ty);) => {
        $(#[$meta])*
        #[derive(Debug, Default)]
        $vis struct $name(pub(crate) slotmap::SlotMap<$key, $item>);

        impl std::ops::Deref for $name {
            type Target = slotmap::SlotMap<$key, $item>;
            #[inline] fn deref(&self) -> &Self::Target { &self.0 }
        }
        impl std::ops::DerefMut for $name {
            #[inline] fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
        }
    };
}

#[macro_export]
macro_rules! define_secondary {
    ($(#[$meta:meta])* $vis:vis struct $name:ident($item:ty);) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone)]
        $vis struct $name(pub(crate) slotmap::SecondaryMap<$crate::EntityId, $item>);

        impl $crate::MichiuSoA for $name {
            type Item = $item;
            #[inline] fn get(&self, id: $crate::EntityId) -> Option<&Self::Item> { self.0.get(id) }
            #[inline] fn get_mut(&mut self, id: $crate::EntityId) -> Option<&mut Self::Item> { self.0.get_mut(id) }
        }

        impl std::ops::Deref for $name {
            type Target = slotmap::SecondaryMap<$crate::EntityId, $item>;
            #[inline] fn deref(&self) -> &Self::Target { &self.0 }
        }
        impl std::ops::DerefMut for $name {
            #[inline] fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
        }
    };

    ($(#[$meta:meta])* $vis:vis struct $name:ident($key:ty => $item:ty);) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone)]
        $vis struct $name(pub(crate) slotmap::SecondaryMap<$key, $item>);

        impl std::ops::Deref for $name {
            type Target = slotmap::SecondaryMap<$key, $item>;
            #[inline] fn deref(&self) -> &Self::Target { &self.0 }
        }
        impl std::ops::DerefMut for $name {
            #[inline] fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
        }
    };
}

#[macro_export]
macro_rules! define_sparse_secondary {
    ($(#[$meta:meta])* $vis:vis struct $name:ident($item:ty);) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone)]
        $vis struct $name(pub(crate) slotmap::SparseSecondaryMap<$crate::EntityId, $item>);

        impl $crate::MichiuSoA for $name {
            type Item = $item;
            #[inline] fn get(&self, id: $crate::EntityId) -> Option<&Self::Item> { self.0.get(id) }
            #[inline] fn get_mut(&mut self, id: $crate::EntityId) -> Option<&mut Self::Item> { self.0.get_mut(id) }
        }

        impl std::ops::Deref for $name {
            type Target = slotmap::SparseSecondaryMap<$crate::EntityId, $item>;
            #[inline] fn deref(&self) -> &Self::Target { &self.0 }
        }
        impl std::ops::DerefMut for $name {
            #[inline] fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
        }
    };

    ($(#[$meta:meta])* $vis:vis struct $name:ident($key:ty => $item:ty);) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone)]
        $vis struct $name(pub(crate) slotmap::SparseSecondaryMap<$key, $item>);

        impl std::ops::Deref for $name {
            type Target = slotmap::SparseSecondaryMap<$key, $item>;
            #[inline] fn deref(&self) -> &Self::Target { &self.0 }
        }
        impl std::ops::DerefMut for $name {
            #[inline] fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
        }
    };
}
