//! UI Elements - the building blocks of the UI

mod canvas;
mod div;
pub mod element;
mod image;
mod input;
mod list;
mod scroll_view;
mod spinner;
mod table;
pub mod text;
mod text_area;
mod view_element;
mod virtual_list;

pub use canvas::{Canvas, Painter, canvas};
pub use div::{Div, div};
pub use element::{AnyElement, Capabilities, Element, EventResult, IntoElement, Render, StableId};
pub use image::{Image, ImageFit, ImageSource, image};
pub use input::{Input, InputType, input};
pub use list::{List, ListItem, ListStyle, list, ordered_list, unordered_list};
pub use scroll_view::{ScrollDirection, ScrollView, scroll_view};
pub use spinner::{Spinner, SpinnerType, spinner};
pub use table::{Table, TableCell, TableRow, cell, header_row, row, table};
pub use text::{StyledText, Text, TextSpan, span, styled_text, text};
pub use text_area::{TextArea, TextAreaState, text_area};
pub use view_element::{ViewElement, view};
pub use virtual_list::{VirtualList, VirtualListHandle, virtual_list};
