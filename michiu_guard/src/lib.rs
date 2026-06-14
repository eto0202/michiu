#![cfg_attr(docsrs, feature(doc_cfg))]
//! # michiu_guard
//!
//! To myself tomorrow...
//!
//! The sentinel of the `michiu` GUI ecosystem.
//!
//! As the name suggests, `michiu_guard` is built specifically to protect the core
//! of the `michiu` GUI framework from untrusted external inputs.
//!
//! ## Design Philosophy: The OS Input Firewall
//!
//! GUI applications continuously receive a stream of untrusted, asynchronous data from the OS
//! (e.g., keyboard events, mouse coordinates, clipboard texts, and drag-and-dropped file paths).
//!
//! To prevent bugs and security vulnerabilities at the framework level, `michiu` establishes
//! a strict architectural boundary:
//!
//! 1. **Callbacks and Event Handlers** must always yield an [`Unvalidated<T>`] wrapper.
//! 2. **Framework Internals and Core APIs** must always consume a [`Validated<T>`] wrapper.
//!
//! This compile-time contract forces developers to explicitly pre-process (using [`map`](Unvalidated::map))
//! and validate (using [`validate_with`](Unvalidated::validate_with)) any raw OS-derived data
//! before it can enter and mutate the core state of the `michiu` framework.
//!
//! ## Safety and Invariants: No Mutable Access
//!
//! Once data is validated, its internal state must remain consistent with the validated invariants.
//! To prevent verified data from being modified outside the validation pipeline:
//!
//! * [`Validated<T>`] implements [`Deref`] but **does not** implement `DerefMut` or `AsMut`.
//! * Once wrapped in [`Validated<T>`], the value is read-only.
//!
//! If you need to mutate a validated value, you must consume the wrapper to retrieve the raw value
//! (using [`into_inner`](Validated::into_inner)), perform your mutations, and pass it back
//! through the validation process as an [`Unvalidated<T>`]. This strict boundary guarantees that
//! invalid states cannot sneak into your core application logic.
//!
//! ## Design Decisions: Why `Unvalidated<T>` Lacks Mutable Access
//!
//! You might notice that [`Unvalidated<T>`] also does not implement `DerefMut` or `AsMut`.
//! This is a deliberate architectural decision, not an oversight.
//!
//! Allowing direct mutable access (e.g., yielding a `&mut T`) to unvalidated data would make
//! it easy to perform implicit, hidden mutations on untrusted inputs. This would heavily compromise
//! code auditability, making it difficult to trace exactly where and how raw external data is
//! being altered before validation.
//!
//! To sanitize, normalize, or otherwise modify raw input, `michiu_guard` enforces two highly
//! visible, explicit pathways:
//!
//! 1. **Functional Pipeline**: Use [`map`](Unvalidated::map) to cleanly transform the inner value.
//! 2. **Explicit Unwrapping**: Consume the wrapper with [`into_inner`](Unvalidated::into_inner)
//!    to recover the raw type `T`, perform your procedural mutations, and then repackage it
//!    back into an [`Unvalidated<T>`].
//!
//! By making raw data mutation an explicit "ritual" rather than an implicit side-effect,
//! we ensure that every single manipulation of untrusted data is clear, intentional, and
//! extremely easy to track during security audits and code reviews.
//!
//! ### A Note on Interior Mutability
//!
//! While `Validated<T>` does not implement `DerefMut` or `AsMut` to prevent direct mutation,
//! it cannot statically prevent state changes if `T` itself employs interior mutability
//! (e.g., types like `Cell`, `RefCell`, `Mutex`, `RwLock`, or atomic types).
//!
//! Because shared references (`&T`) to these types still allow internal mutations, wrapping them
//! in `Validated<T>` can lead to validated invariants being bypassed or broken. To guarantee
//! absolute safety, it is strongly recommended to avoid wrapping any types that exhibit interior
//! mutability within `Validated<T>`.
//!
//! ## Quick Start
//!
//! Here is a complete example of how `michiu_guard` protects a file-drop event
//! in the `michiu` framework.
//!
//! ```rust
//! # use std::path::PathBuf;
//! # use michiu_guard::{Unvalidated, Validated};
//! # // Mock function representing the framework's internal API
//! # fn open_file(path: Validated<PathBuf>) {}
//! // 1. An OS event callback yields an `Unvalidated` path.
//! fn on_file_drop(unvalidated_path: Unvalidated<PathBuf>) {
//!
//!     // 2. The developer must validate it before passing it to the framework.
//!     let validated_path = unvalidated_path.validate_with(|path| {
//!         if path.exists() && path.is_file() {
//!             Ok(path)
//!         } else {
//!             Err("Invalid file dropped")
//!         }
//!     });
//!
//!     // 3. The framework only accepts `Validated<T>`, preventing accidental reuse of untrusted paths.
//!     if let Ok(safe_path) = validated_path {
//!         open_file(safe_path);
//!     }
//! }
//! ```
//! ## Integration with Serde (Optional)
//!
//! When the `serde` feature is enabled, `michiu_guard` automatically integrates with
//! the `serde` ecosystem to provide secure, on-the-fly validation during deserialization.
//!
//! * **[`Unvalidated<T>`]**: Implements `Deserialize`. Raw external inputs (e.g., from network APIs or configuration files)
//!   can be safely deserialized as unvalidated data.
//! * **[`Validated<T>`]**: Implements `Deserialize` (requires `T: Validate`). When deserializing directly
//!   into `Validated<T>`, the verification step is automatically executed. If validation fails,
//!   the deserialization itself will fail, preventing invalid data from ever being instantiated inside your application.
//!
//! To use this, enable the `serde` feature in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! michiu_guard = { version = "...", features = ["serde"] }
//! ```

use std::{
    borrow::Borrow,
    ffi::{CStr, CString, OsStr, OsString},
    fmt,
    ops::Deref,
    path::{Path, PathBuf},
};

/// A wrapper representing a value that has not yet been validated.
///
/// This wrapper ensures that raw, potentially untrusted input is kept distinct
/// from validated data until it undergoes verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Unvalidated<T>(T);

impl<T> Unvalidated<T> {
    /// Creates a new `Unvalidated` wrapper containing the raw value.
    #[inline]
    pub fn new(val: T) -> Self {
        Self(val)
    }

    /// Consumes the wrapper and returns the underlying unvalidated value.
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }

    /// Promotes the unvalidated value to a [`Validated`] wrapper without performing any checks.
    ///
    /// # Caution
    /// This method bypasses validation. Use this only when you are certain
    /// that the value already meets all required validation invariants (for example,
    /// when restoring verified data from a trusted database).
    #[inline]
    pub fn assume_valid(self) -> Validated<T> {
        Validated::new_unchecked(self.0)
    }

    /// Validates the wrapped value using the provided validator function.
    ///
    /// Returns a [`Validated<U>`] wrapper if the validator succeeds, or an error of type `E`
    /// if it fails.
    ///
    /// This method supports "Parse, don't validate" patterns, allowing you to transform
    /// raw representations (like `String`) into more specialized, safer types (like custom structs or `PathBuf`)
    /// during validation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use michiu_guard::{Unvalidated, Validated};
    /// # use std::path::PathBuf;
    /// # let exe_path_str = std::env::current_exe().unwrap().to_string_lossy().into_owned();
    /// // An OS file-drop event yields an unvalidated path string.
    /// let raw_path_str = Unvalidated::new(exe_path_str);
    ///
    /// // Validate and transform `String` into `PathBuf` in a single step
    /// let validated_path = raw_path_str.validate_with(|path_str| {
    ///     let path = PathBuf::from(path_str);
    ///     if path.exists() && path.is_file() {
    ///         Ok(path)
    ///     } else {
    ///         Err("Dropped path must be an existing file")
    ///     }
    /// });
    ///
    /// assert!(validated_path.is_ok());
    /// ```
    #[inline]
    pub fn validate_with<U, E>(
        self,
        validator: impl FnOnce(T) -> Result<U, E>,
    ) -> Result<Validated<U>, E> {
        let validated_inner = validator(self.0)?;
        Ok(Validated(validated_inner))
    }

    /// Validates and transforms the value, returning the original `Unvalidated<T>` back on failure.
    ///
    /// This is useful for heavy, expensive-to-clone types where you need to recover
    /// the raw input if validation fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use michiu_guard::{Unvalidated, Validated};
    /// // Raw input from clipboard (missing the required prefix)
    /// let raw_input = Unvalidated::new("michiu.org".to_string());
    ///
    /// // 1. Attempt validation (fails because it doesn't start with https://)
    /// let validated = raw_input.try_validate_with(|url| {
    ///     if url.starts_with("https://") {
    ///         Ok(url)
    ///     } else {
    ///         Err(("URL must start with https://", url))
    ///     }
    /// });
    ///
    /// assert!(validated.is_err());
    /// let (err, recovered) = validated.unwrap_err();
    /// assert_eq!(err, "URL must start with https://");
    ///
    /// // 2. Recover the raw data, sanitize it, and retry validation
    /// let fixed = recovered.map(|url| format!("https://{}", url));
    /// let retry = fixed.try_validate_with(|url| {
    ///     if url.starts_with("https://") {
    ///         Ok(url)
    ///     } else {
    ///         Err(("URL must start with https://", url))
    ///     }
    /// });
    ///
    /// assert!(retry.is_ok());
    /// assert_eq!(retry.unwrap().into_inner(), "https://michiu.org");
    /// ```
    #[inline]
    pub fn try_validate_with<U, E>(
        self,
        validator: impl FnOnce(T) -> Result<U, (E, T)>,
    ) -> Result<Validated<U>, (E, Self)> {
        match validator(self.0) {
            Ok(val) => Ok(Validated(val)),
            Err((err, raw)) => Err((err, Unvalidated(raw))),
        }
    }

    /// Validates the value by reference without type conversion, returning the original `Unvalidated<T>` back on failure.
    ///
    /// This allows validating a type in-place using its shared reference, without copying the data,
    /// and ensures the raw data is preserved if validation fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use michiu_guard::{Unvalidated, Validated};
    /// // An unvalidated password input
    /// let raw_password = Unvalidated::new("secret".to_string());
    ///
    /// // Validate by reference, checking if it meets length requirements
    /// let result = raw_password.try_validate_ref(|p| {
    ///     if p.len() >= 8 {
    ///         Ok(())
    ///     } else {
    ///         Err("Password must be at least 8 characters long")
    ///     }
    /// });
    ///
    /// assert!(result.is_err());
    /// let (err, recovered) = result.unwrap_err();
    /// assert_eq!(err, "Password must be at least 8 characters long");
    ///
    /// // Since validation failed, the original raw data is safely returned
    /// assert_eq!(recovered.into_inner(), "secret");
    /// ```
    #[inline]
    pub fn try_validate_ref<E>(
        self,
        validator: impl FnOnce(&T) -> Result<(), E>,
    ) -> Result<Validated<T>, (E, Self)> {
        match validator(&self.0) {
            Ok(()) => Ok(Validated(self.0)),
            Err(e) => Err((e, self)),
        }
    }

    /// Maps an `Unvalidated<T>` to `Unvalidated<U>` by applying a function to the contained value.
    ///
    /// This is particularly useful for sanitizing or normalizing raw OS-derived inputs
    /// (such as trimming clipboard text or truncating oversized keyboard inputs)
    /// before validation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use michiu_guard::{Unvalidated, Validated};
    /// // Raw text pasted from the OS clipboard (contains untrusted spaces and newlines)
    /// let raw_clipboard = Unvalidated::new("  https://michiu.org/docs  \n".to_string());
    ///
    /// // 1. Use `map` to pre-process (trim) the raw clipboard input.
    /// let cleaned_input = raw_clipboard.map(|s| s.trim().to_string());
    /// assert_eq!(cleaned_input.into_inner(), "https://michiu.org/docs");
    ///
    /// // 2. Alternatively, chain the methods to pre-process and validate
    /// //    in a single, fluent pipeline.
    /// let raw_clipboard_to_chain = Unvalidated::new("  https://michiu.org  \n".to_string());
    /// let validated_url = raw_clipboard_to_chain
    ///     .map(|text| text.trim().to_string())
    ///     .validate_with(|url| {
    ///         if url.starts_with("https://") {
    ///             Ok(url)
    ///         } else {
    ///             Err("Clipboard must contain a secure URL")
    ///         }
    ///     });
    ///
    /// assert!(validated_url.is_ok());
    /// assert_eq!(validated_url.unwrap().into_inner(), "https://michiu.org");
    /// ```
    #[inline]
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Unvalidated<U> {
        Unvalidated(f(self.0))
    }
}

impl<T> AsRef<T> for Unvalidated<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.0
    }
}

/// A wrapper representing a value that has been successfully validated.
///
/// Under normal circumstances, you cannot construct this wrapper directly with unvalidated raw data.
/// Instead, you must use [`Unvalidated::validate_with`] or implement [`Validate`] and use [`TryFrom`].
///
/// `Validated<T>` guarantees that the contained value has passed validation and cannot be mutated
/// from outside (as it does not implement `DerefMut` or `AsMut`), preserving its safety invariants.
///
/// # Warning: Interior Mutability
///
/// While `Validated<T>` prevents direct mutable access, Rust's type system allows
/// types with interior mutability (such as `Cell`, `RefCell`, `Mutex`, `RwLock`, or
/// atomic types) to be mutated through shared references (`&T`).
///
/// To strictly preserve validated invariants and prevent unexpected state corruption,
/// do not wrap types that utilize interior mutability inside `Validated<T>`.
///
/// # Examples
///
/// ```rust
/// # use michiu_guard::Validated;
/// // Under normal flow, you get a `Validated` wrapper from validation.
/// // Once wrapped, you can easily read the inner value using `Deref`:
/// let validated = Validated::new_unchecked("safe text".to_string());
/// assert_eq!(validated.len(), 9);
/// assert_eq!(*validated, "safe text");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Validated<T>(T);

impl<T> Validated<T> {
    /// Creates a new [`Validated`] wrapper without performing any validation checks.
    ///
    /// Under normal circumstances, prefer using [`validate_with`](Unvalidated::validate_with)
    /// or the [`TryFrom`] implementation on [`Unvalidated`].
    #[inline]
    pub fn new_unchecked(val: T) -> Self {
        Self(val)
    }

    /// Consumes the wrapper and returns the underlying validated value.
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for Validated<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A trait for types that can validate themselves.
///
/// Implementing this trait enables automatic conversion from [`Unvalidated<T>`]
/// to [`Validated<T>`] using [`TryFrom`].
///
/// # Examples
///
/// Here is how you can implement `Validate` to verify dropped assets in the GUI:
///
/// ```rust
/// # use michiu_guard::{Validate, Unvalidated, Validated};
/// # use std::path::PathBuf;
/// #[derive(Debug, PartialEq, Eq)]
/// struct DroppedImage(PathBuf);
///
/// impl Validate for DroppedImage {
///     type Error = &'static str;
///
///     fn validate(self) -> Result<Self, Self::Error> {
///         // Verify that the dropped file has an allowed image extension
///         let ext = self.0.extension().and_then(|e| e.to_str()).unwrap_or("");
///         match ext {
///             "png" | "jpg" | "jpeg" => Ok(self),
///             _ => Err("Only PNG and JPEG images are supported"),
///         }
///     }
/// }
///
/// // Convert the unvalidated OS drop event into a validated image asset
/// let raw_event = Unvalidated::new(DroppedImage(PathBuf::from("avatar.png")));
/// let validated: Validated<DroppedImage> = raw_event.try_into().unwrap();
///
/// assert_eq!(validated.into_inner(), DroppedImage(PathBuf::from("avatar.png")));
/// ```
pub trait Validate: Sized {
    /// The error type returned if validation fails.
    type Error;

    /// Validates the type, returning the validated self or an error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use michiu_guard::Validate;
    /// # struct Age(i32);
    /// # impl Validate for Age {
    /// #     type Error = &'static str;
    /// #     fn validate(self) -> Result<Self, Self::Error> {
    /// #         if self.0 >= 0 { Ok(self) } else { Err("Negative age") }
    /// #     }
    /// # }
    /// let raw_age = Age(17);
    ///
    /// // Call the validation check directly on the type
    /// let result = raw_age.validate();
    /// assert!(result.is_ok());
    /// ```
    fn validate(self) -> Result<Self, Self::Error>;

    /// Convenience method to validate the value and wrap it in [`Validated`].
    /// # Examples
    ///
    /// ```rust
    /// # use michiu_guard::{Validate, Validated};
    /// # #[derive(Debug, PartialEq)]
    /// # struct Age(i32);
    /// # impl Validate for Age {
    /// #     type Error = &'static str;
    /// #     fn validate(self) -> Result<Self, Self::Error> {
    /// #         if self.0 >= 0 { Ok(self) } else { Err("Negative age") }
    /// #     }
    /// # }
    /// let raw_age = Age(17);
    ///
    /// // Validate and wrap in `Validated` in a single step
    /// let validated: Validated<Age> = raw_age.validate_into().unwrap();
    /// assert_eq!(validated.into_inner(), Age(17));
    /// ```
    #[inline]
    fn validate_into(self) -> Result<Validated<Self>, Self::Error> {
        Unvalidated::new(self).try_into()
    }
}

impl<T: Validate> TryFrom<Unvalidated<T>> for Validated<T> {
    type Error = T::Error;

    /// # Examples
    ///
    /// ```rust
    /// # use michiu_guard::{Unvalidated, Validated, Validate};
    /// # #[derive(Debug, PartialEq)]
    /// # struct Age(i32);
    /// # impl Validate for Age {
    /// #     type Error = &'static str;
    /// #     fn validate(self) -> Result<Self, Self::Error> {
    /// #         if self.0 >= 0 { Ok(self) } else { Err("Negative age") }
    /// #     }
    /// # }
    /// let unvalidated = Unvalidated::new(Age(17));
    /// let validated: Validated<Age> = unvalidated.try_into().unwrap();
    ///
    /// assert_eq!(validated.into_inner(), Age(17));
    /// ```
    #[inline]
    fn try_from(unvalidated: Unvalidated<T>) -> Result<Self, Self::Error> {
        let raw = unvalidated.0;
        let validated_inner = raw.validate()?;
        Ok(Self(validated_inner))
    }
}

impl<T> AsRef<T> for Validated<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> Borrow<T> for Validated<T> {
    #[inline]
    fn borrow(&self) -> &T {
        &self.0
    }
}

impl Borrow<str> for Validated<String> {
    #[inline]
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl Borrow<Path> for Validated<PathBuf> {
    #[inline]
    fn borrow(&self) -> &Path {
        &self.0
    }
}

impl<T> Borrow<[T]> for Validated<Vec<T>> {
    #[inline]
    fn borrow(&self) -> &[T] {
        &self.0
    }
}

impl Borrow<OsStr> for Validated<OsString> {
    #[inline]
    fn borrow(&self) -> &OsStr {
        &self.0
    }
}

impl Borrow<CStr> for Validated<CString> {
    #[inline]
    fn borrow(&self) -> &CStr {
        &self.0
    }
}

impl<T: fmt::Display> fmt::Display for Validated<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<T: fmt::Display> fmt::Display for Unvalidated<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[cfg(any(feature = "serde", test))]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
/// Deserializes raw data directly into the wrapper without running any validation.
///
/// Use this when you want to defer validation or keep raw, potentially untrusted input
/// in your data structures for preprocessing before validating it.
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for Unvalidated<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Unvalidated::new)
    }
}

#[cfg(any(feature = "serde", test))]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
/// Deserializes a value and automatically executes its [`Validate::validate`] implementation.
///
/// This implementation ensures that you cannot bypass validation when instantiating
/// a `Validated<T>` from external serialized data (like JSON or YAML).
///
/// # Examples
///
/// ```rust
/// # use michiu_guard::{Validate, Validated};
/// # use serde::Deserialize;
/// #
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Username(String);
///
/// impl Validate for Username {
///     type Error = &'static str;
///
///     fn validate(self) -> Result<Self, Self::Error> {
///         if self.0.len() >= 3 {
///             Ok(self)
///         } else {
///             Err("Username must be at least 3 characters long")
///         }
///     }
/// }
///
/// // 1. If the raw data is valid, deserialization succeeds.
/// let ok_json = r#""Michiu""#;
/// let validated: Validated<Username> = serde_json::from_str(ok_json).unwrap();
/// assert_eq!((*validated).0, "Michiu");
///
/// // 2. If validation fails, deserialization fails with the validation error.
/// let bad_json = r#""Me""#;
/// let result: Result<Validated<Username>, serde_json::Error> = serde_json::from_str(bad_json);
/// assert!(result.is_err());
/// assert!(result.unwrap_err().to_string().contains("Username must be at least 3 characters long"));
/// ```
impl<'de, T> serde::Deserialize<'de> for Validated<T>
where
    T: serde::Deserialize<'de> + Validate,
    T::Error: std::fmt::Display,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = T::deserialize(deserializer)?;
        raw.validate_into().map_err(serde::de::Error::custom)
    }
}

#[cfg(any(feature = "serde", test))]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
/// Serializes the validated value by delegating directly to the underlying type `T`.
///
/// This implementation is transparent, meaning the `Validated<T>` wrapper itself is
/// omitted in the serialized output. For example, serializing a `Validated<String>`
/// containing `"safe_text"` yields a plain JSON string `"safe_text"`, rather than
/// a nested object or tuple structure.
///
/// This allows you to safely serialize validated domain models for network transmission
/// or database storage without leaking internal wrapper details.
///
/// # Examples
///
/// ```rust
/// # use michiu_guard::Validated;
/// let validated = Validated::new_unchecked("safe_text".to_string());
/// let json = serde_json::to_string(&validated).unwrap();
///
/// // The serialized output is identical to the inner String
/// assert_eq!(json, r#""safe_text""#);
/// ```
impl<T: serde::Serialize> serde::Serialize for Validated<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

#[cfg(any(feature = "serde", test))]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
/// Serializes the unvalidated value by delegating directly to the underlying type `T`.
///
/// /// # Warning
///
/// This will serialize the raw, unvalidated data as-is. Use this only when
/// writing raw inputs to temporary storage (such as auto-saved drafts) or for
/// debugging/logging purposes. Do not use this when sending data to production
/// databases or trusted external APIs.
///
/// Just like [`Validated<T>`], this implementation is transparent. The `Unvalidated<T>`
/// wrapper is omitted in the serialized output, ensuring that raw inputs can be written out
/// in their original data format.
///
/// # Examples
///
/// ```rust
/// # use michiu_guard::Unvalidated;
/// let unvalidated = Unvalidated::new(42);
/// let json = serde_json::to_string(&unvalidated).unwrap();
///
/// // Serializes directly to the raw inner value
/// assert_eq!(json, "42");
/// ```
impl<T: serde::Serialize> serde::Serialize for Unvalidated<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}
