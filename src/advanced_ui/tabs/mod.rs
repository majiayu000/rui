mod model;
mod tab_list;
mod tab_panel;
mod tabs_root;

use crate::elements::element::AnyElement;

pub use model::Tab;
pub use tab_list::TabList;
pub use tab_panel::TabPanel;
pub use tabs_root::Tabs;

pub fn tab(value: impl Into<String>, label: impl Into<String>) -> Tab {
    Tab::new(value, label)
}

pub fn tab_list<I, T>(tabs: I, selected: impl Into<String>) -> TabList
where
    I: IntoIterator<Item = T>,
    T: Into<Tab>,
{
    TabList::new(tabs, selected)
}

pub fn tab_panel(value: impl Into<String>, child: impl Into<AnyElement>) -> TabPanel {
    TabPanel::new(value, child)
}

pub fn tabs<I, T>(tabs: I, selected: impl Into<String>) -> Tabs
where
    I: IntoIterator<Item = T>,
    T: Into<Tab>,
{
    Tabs::new(tabs, selected)
}
