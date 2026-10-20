//! course 层页面（`course` feature，docs/FEATURE_TIERS.md）。未编译时由
//! `absent_pages!` 生成同名 404 组件，`Route` 不变。

use dioxus::prelude::*;

use crate::components::admin_entitlements::AdminEntitlementsPage;
use crate::components::module_gate::ModuleGate;
use crate::components::view::Container;
use module_course::course::{CourseDetailPage, CoursesIndexPage, LessonPage, MyAnnotationsPage};
#[cfg(feature = "payments")]
use module_course::pay_ui::MyOrdersPage;
use module_forum::forum::DiscussionPanel;

#[component]
pub fn Courses() -> Element {
  rsx! { ModuleGate { id: "course".to_string(), CoursesIndexPage {} } }
}

#[component]
pub fn CourseDetail(slug: String) -> Element {
  rsx! { ModuleGate { id: "course".to_string(), CourseDetailPage { slug: slug } } }
}

#[component]
pub fn Lesson(slug: String, chapter: String, lesson: String) -> Element {
  let lesson_path = format!("{}/{}/{}", slug, chapter, lesson);
  rsx! {
      ModuleGate { id: "course".to_string(),
          LessonPage { slug: slug.clone(), chapter: chapter.clone(), lesson: lesson.clone() }
          // 资源讨论面板：关联论坛话题（以 lesson kind 记录）
          Container {
              DiscussionPanel {
                  resource_kind: "lesson".to_string(),
                  resource_path: lesson_path,
              }
          }
      }
  }
}

/// /me/orders 我的订单页（payments feature 门控；关闭时渲染占位提示，
/// 路由保留以免旧链接 404）。
#[component]
pub fn MyOrders() -> Element {
  #[cfg(feature = "payments")]
  {
    rsx! { ModuleGate { id: "course".to_string(), MyOrdersPage {} } }
  }
  #[cfg(not(feature = "payments"))]
  {
    rsx! {
        section { class: "py-20 bg-white dark:bg-slate-950 min-h-[60vh]",
            div { class: "mx-auto max-w-4xl px-4 text-center text-slate-500",
                "在线支付未开通，暂无订单可查。如需开通课程权益请联系管理员。"
            }
        }
    }
  }
}

/// /me/annotations 个人标注列表页
#[component]
pub fn MyAnnotations() -> Element {
  rsx! { MyAnnotationsPage {} }
}

#[component]
pub fn AdminEntitlements() -> Element {
  rsx! { AdminEntitlementsPage {} }
}
