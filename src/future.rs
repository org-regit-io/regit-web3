// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Target-aware bounds for replaceable capabilities and their futures.
//!
//! Native capability futures implement [`Send`]. JavaScript WebAssembly hosts
//! keep their futures on the originating thread because browser values cannot
//! cross native thread boundaries.

/// Requires [`Send`] except on JavaScript `wasm32-unknown-unknown` hosts.
///
/// Capability return types combine this marker with [`std::future::Future`].
/// The blanket implementation preserves ordinary native `Send` guarantees
/// while allowing browser promises and other host-local futures in WebAssembly.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub trait MaybeSend: Send {}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
impl<T: Send + ?Sized> MaybeSend for T {}

/// Allows host-local futures on JavaScript `wasm32-unknown-unknown` hosts.
///
/// On other targets this marker also requires [`Send`].
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub trait MaybeSend {}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
impl<T: ?Sized> MaybeSend for T {}

/// Requires [`Sync`] except on JavaScript `wasm32-unknown-unknown` hosts.
///
/// Composed capabilities use this marker when a shared client reference must
/// cross native thread boundaries. JavaScript clients stay on the originating
/// host thread and may contain browser values that do not implement `Sync`.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub trait MaybeSync: Sync {}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
impl<T: Sync + ?Sized> MaybeSync for T {}

/// Allows host-local shared clients on JavaScript `wasm32-unknown-unknown` hosts.
///
/// On other targets this marker also requires [`Sync`].
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub trait MaybeSync {}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
impl<T: ?Sized> MaybeSync for T {}
