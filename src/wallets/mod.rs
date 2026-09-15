// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Interface-neutral wallet and signing boundary.
//!
//! Generic signing requests, external-wallet handoff, and verification that a
//! signed payload matches the requested transaction belong here.
//! Consumers select wallet backends, provision secrets, and own approval policy.
//! Read operations must never construct a signer or load keys.
//! No signer or wallet connector is implemented yet.
