//! File dialogs: `NSOpenPanel` and `NSSavePanel` for the prompts the app
//! asked for with [`AppContext::prompt_for_paths`] and
//! [`AppContext::prompt_for_new_path`].
//!
//! Each panel runs modally from the event loop, after the windows ran, and its
//! answer completes the prompt's task, which runs in the next frame.

use crate::core::app::AppContext;
use crate::core::prompt::{PathPrompt, PathPromptOptions};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::{NSArray, NSString};
use std::path::{Path, PathBuf};

/// `NSModalResponseOK`: the user chose.
const MODAL_RESPONSE_OK: isize = 1;

/// Show the prompts the app asked for, one after another.
pub(crate) fn run_path_prompts(context: &mut AppContext) {
    for prompt in context.take_path_prompts() {
        match prompt {
            PathPrompt::Open { options, answer } => answer.send(run_open_panel(options)),
            PathPrompt::Save { directory, answer } => answer.send(run_save_panel(&directory)),
        }
    }
}

fn run_open_panel(options: PathPromptOptions) -> Option<Vec<PathBuf>> {
    let panel: Retained<AnyObject> = unsafe { msg_send![class!(NSOpenPanel), openPanel] };
    unsafe {
        let _: () = msg_send![&*panel, setCanChooseFiles: options.files];
        let _: () = msg_send![&*panel, setCanChooseDirectories: options.directories];
        let _: () = msg_send![&*panel, setAllowsMultipleSelection: options.multiple];
        let _: () = msg_send![&*panel, setResolvesAliases: true];
    }
    if !run_modal(&panel) {
        return None;
    }
    let urls: Retained<NSArray<AnyObject>> = unsafe { msg_send![&*panel, URLs] };
    Some(urls.iter().filter_map(|url| url_path(&url)).collect())
}

fn run_save_panel(directory: &Path) -> Option<PathBuf> {
    let panel: Retained<AnyObject> = unsafe { msg_send![class!(NSSavePanel), savePanel] };
    let directory = NSString::from_str(&directory.to_string_lossy());
    unsafe {
        let url: Option<Retained<AnyObject>> =
            msg_send![class!(NSURL), fileURLWithPath: &*directory, isDirectory: true];
        if let Some(url) = url {
            let _: () = msg_send![&*panel, setDirectoryURL: &*url];
        }
        let _: () = msg_send![&*panel, setCanCreateDirectories: true];
    }
    if !run_modal(&panel) {
        return None;
    }
    let url: Option<Retained<AnyObject>> = unsafe { msg_send![&*panel, URL] };
    match url.and_then(|url| url_path(&url)) {
        Some(path) => Some(path),
        None => {
            log::error!("macOS save panel was confirmed without a file path");
            None
        }
    }
}

/// Run `panel` until the user confirms or cancels it; returns whether they confirmed.
fn run_modal(panel: &AnyObject) -> bool {
    let response: isize = unsafe { msg_send![panel, runModal] };
    response == MODAL_RESPONSE_OK
}

fn url_path(url: &AnyObject) -> Option<PathBuf> {
    let path: Option<Retained<NSString>> = unsafe { msg_send![url, path] };
    path.map(|path| PathBuf::from(path.to_string()))
}
