//! File dialogs the app asked for, waiting for the platform to show them.

use super::AppContext;
use crate::core::executor::Task;
use crate::core::prompt::{PathPrompt, PathPromptOptions, PromptDismissed, prompt_reply};
use std::path::{Path, PathBuf};

impl AppContext {
    /// Ask the user for existing paths. The task finishes with the chosen
    /// paths, or `None` when the user cancels.
    ///
    /// ```ignore
    /// let paths = cx.prompt_for_paths(PathPromptOptions::default());
    /// cx.spawn(move |cx| async move {
    ///     if let Ok(Some(paths)) = paths.await {
    ///         cx.update(|cx| open(cx, paths));
    ///     }
    /// })
    /// .detach();
    /// ```
    ///
    /// # Panics
    ///
    /// When `options` allow neither files nor directories.
    pub fn prompt_for_paths(
        &mut self,
        options: PathPromptOptions,
    ) -> Task<Result<Option<Vec<PathBuf>>, PromptDismissed>> {
        assert!(
            options.files || options.directories,
            "a path prompt must let the user choose files or directories"
        );
        let (answer, reply) = prompt_reply();
        self.path_prompts.push(PathPrompt::Open { options, answer });
        self.spawn(move |_| reply)
    }

    /// Ask the user for a path to save to, starting in `directory`. The task
    /// finishes with the path, or `None` when the user cancels.
    pub fn prompt_for_new_path(
        &mut self,
        directory: &Path,
    ) -> Task<Result<Option<PathBuf>, PromptDismissed>> {
        let (answer, reply) = prompt_reply();
        self.path_prompts.push(PathPrompt::Save {
            directory: directory.to_path_buf(),
            answer,
        });
        self.spawn(move |_| reply)
    }

    /// The prompts asked for since the platform last took them, in order.
    pub(crate) fn take_path_prompts(&mut self) -> Vec<PathPrompt> {
        std::mem::take(&mut self.path_prompts)
    }
}
