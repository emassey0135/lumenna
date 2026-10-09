//! Somewhere to go in the main window, and the sidebar that lists them: the surface's
//! (`lumenna_surface::places`), under the names the desktop apps use.

pub use lumenna_surface::places::{
    Place, SidebarEntry as Entry, SidebarGroup as Group, SidebarKind as Kind, sidebar,
};
