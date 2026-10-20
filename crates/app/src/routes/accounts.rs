//! accounts 层页面（`accounts` feature，docs/FEATURE_TIERS.md）：论坛、我的话题、
//! 管理后台。未编译时由 `absent_pages!` 生成同名 404 组件，`Route` 不变。

use dioxus::prelude::*;

use crate::components::module_gate::ModuleGate;
use module_admin::admin::{
  AdminCommentsPage, AdminDashboardPage, AdminModerationPage, AdminModerationSettingsPage,
  AdminTopicsPage, AdminUsersPage,
};
use module_forum::forum::{
  MyTopicsPage, NewTopicPage, TopicDetailPage, TopicsByTagPage, TopicsIndexPage,
};

#[component]
pub fn TopicsIndex() -> Element {
  rsx! { ModuleGate { id: "forum".to_string(), TopicsIndexPage {} } }
}

#[component]
pub fn TopicsNew() -> Element {
  rsx! { ModuleGate { id: "forum".to_string(), NewTopicPage {} } }
}

#[component]
pub fn TopicsByTag(tag: String) -> Element {
  rsx! { ModuleGate { id: "forum".to_string(), TopicsByTagPage { tag } } }
}

#[component]
pub fn TopicDetail(id: i32) -> Element {
  rsx! { ModuleGate { id: "forum".to_string(), TopicDetailPage { id } } }
}

#[component]
pub fn MyTopics() -> Element {
  rsx! { ModuleGate { id: "forum".to_string(), MyTopicsPage {} } }
}

#[component]
pub fn AdminDashboard() -> Element {
  rsx! { AdminDashboardPage {} }
}

#[component]
pub fn AdminUsers() -> Element {
  rsx! { AdminUsersPage {} }
}

#[component]
pub fn AdminComments() -> Element {
  rsx! { AdminCommentsPage {} }
}

#[component]
pub fn AdminTopics() -> Element {
  rsx! { AdminTopicsPage {} }
}

#[component]
pub fn AdminModeration() -> Element {
  rsx! { AdminModerationPage {} }
}

#[component]
pub fn AdminModerationSettings() -> Element {
  rsx! { AdminModerationSettingsPage {} }
}
