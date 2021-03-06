// Copyright 2019 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Protocol versions for various endpoints provided by splinter.

pub mod authorization;
pub mod component;
pub mod service;

// Admin REST API protocol versions
pub const ADMIN_PROTOCOL_VERSION: u32 = 2;

#[cfg(all(feature = "rest-api-actix", feature = "admin-service"))]
pub(crate) const ADMIN_APPLICATION_REGISTRATION_PROTOCOL_MIN: u32 = 1;
#[cfg(all(feature = "rest-api-actix", feature = "admin-service"))]
pub(crate) const ADMIN_SUBMIT_PROTOCOL_MIN: u32 = 1;

#[cfg(all(feature = "rest-api-actix", feature = "admin-service"))]
pub(crate) const ADMIN_FETCH_PROPOSALS_PROTOCOL_MIN: u32 = 1;

#[cfg(all(feature = "rest-api-actix", feature = "admin-service"))]
pub(crate) const ADMIN_LIST_PROPOSALS_PROTOCOL_MIN: u32 = 1;
#[cfg(all(feature = "rest-api-actix", feature = "admin-service"))]
pub(crate) const ADMIN_LIST_CIRCUITS_MIN: u32 = 1;
#[cfg(all(feature = "rest-api-actix", feature = "admin-service"))]
pub(crate) const ADMIN_FETCH_CIRCUIT_MIN: u32 = 1;

// Admin Service protocol versions
pub const ADMIN_SERVICE_PROTOCOL_VERSION: u32 = 2;

#[cfg(feature = "admin-service")]
pub(crate) const ADMIN_SERVICE_PROTOCOL_MIN: u32 = 1;

// The currently supported circuit version
pub const CIRCUIT_PROTOCOL_VERSION: i32 = 2;

#[cfg(feature = "authorization")]
pub const AUTHORIZATION_PROTOCOL_VERSION: u32 = 1;

#[cfg(all(
    feature = "authorization-handler-maintenance",
    feature = "rest-api-actix"
))]
pub(crate) const AUTHORIZATION_MAINTENANCE_MIN: u32 = 1;

// Authorization (namely RBAC management)  protocol versions
#[cfg(feature = "authorization")]
pub(crate) const AUTHORIZATION_RBAC_ROLES_MIN: u32 = 1;
#[cfg(feature = "authorization")]
pub(crate) const AUTHORIZATION_RBAC_ROLE_MIN: u32 = 1;

#[cfg(feature = "oauth")]
pub const OAUTH_PROTOCOL_VERSION: u32 = 1;

#[cfg(all(feature = "oauth", feature = "rest-api-actix"))]
pub(crate) const OAUTH_CALLBACK_MIN: u32 = 1;
#[cfg(all(feature = "oauth", feature = "rest-api-actix"))]
pub(crate) const OAUTH_LOGIN_MIN: u32 = 1;
#[cfg(all(feature = "oauth", feature = "rest-api-actix"))]
pub(crate) const OAUTH_LOGOUT_MIN: u32 = 1;

#[cfg(feature = "registry")]
pub const REGISTRY_PROTOCOL_VERSION: u32 = 1;

#[cfg(all(feature = "registry", feature = "rest-api-actix"))]
pub(crate) const REGISTRY_LIST_NODES_MIN: u32 = 1;
#[cfg(all(feature = "registry", feature = "rest-api-actix"))]
pub(crate) const REGISTRY_FETCH_NODE_MIN: u32 = 1;

#[cfg(any(
    feature = "biome-credentials",
    feature = "biome-key-management",
    feature = "biome-notifications",
    feature = "biome-oauth"
))]
pub const BIOME_PROTOCOL_VERSION: u32 = 1;

#[cfg(all(feature = "biome-credentials", feature = "rest-api",))]
pub(crate) const BIOME_REGISTER_PROTOCOL_MIN: u32 = 1;
#[cfg(all(feature = "biome-credentials", feature = "rest-api",))]
pub(crate) const BIOME_LOGIN_PROTOCOL_MIN: u32 = 1;
#[cfg(all(feature = "biome-credentials", feature = "rest-api",))]
pub(crate) const BIOME_USER_PROTOCOL_MIN: u32 = 1;
#[cfg(all(feature = "biome-credentials", feature = "rest-api",))]
pub(crate) const BIOME_LIST_USERS_PROTOCOL_MIN: u32 = 1;
#[cfg(all(feature = "biome-credentials", feature = "rest-api"))]
pub(crate) const BIOME_VERIFY_PROTOCOL_MIN: u32 = 1;

#[cfg(all(feature = "biome-key-management", feature = "rest-api",))]
pub(crate) const BIOME_KEYS_PROTOCOL_MIN: u32 = 1;

#[cfg(all(feature = "biome-profile", feature = "rest-api",))]
pub(crate) const BIOME_FETCH_PROFILE_PROTOCOL_MIN: u32 = 1;
#[cfg(all(feature = "biome-profile", feature = "rest-api",))]
pub(crate) const BIOME_FETCH_PROFILES_PROTOCOL_MIN: u32 = 1;
#[cfg(all(feature = "biome-profile", feature = "rest-api",))]
pub(crate) const BIOME_LIST_PROFILES_PROTOCOL_MIN: u32 = 1;
