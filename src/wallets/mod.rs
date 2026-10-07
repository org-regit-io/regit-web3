// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Wallet transaction preparation, review and external signing-handoff contracts.
//!
//! The module scope includes typed unsigned payloads, reviewed-payload binding,
//! and verification extensions for returned signed payloads. Preparation and
//! submission are separate operations; reads never construct a signer or load
//! keys. Signing backends, secrets and approval policy are caller-owned.
//! Preparation/handoff contracts and concrete wallet backends are not implemented
//! yet; backends remain modular extensions.
