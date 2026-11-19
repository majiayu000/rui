use crate::core::app::AppContext;
use crate::core::prompt::PathPrompt;
use rfd::AsyncFileDialog;
use std::cell::Cell;
use std::rc::Rc;

/// Drain the app's queue without blocking the winit event loop. Show a single
/// dialog at a time, including when another window queues a prompt.
pub(super) fn run_path_prompts(context: &mut AppContext, showing: &Rc<Cell<bool>>) {
    if showing.get() {
        return;
    }
    let pending = context.take_path_prompts();
    if pending.is_empty() {
        return;
    }
    showing.set(true);
    let showing = Rc::clone(showing);
    context
        .spawn(move |_| async move {
            for prompt in pending {
                show(prompt).await;
            }
            showing.set(false);
        })
        .detach();
}

async fn show(prompt: PathPrompt) {
    match prompt {
        PathPrompt::Open { options, answer } => {
            if options.files && options.directories {
                log::warn!("winit file dialogs cannot select files and directories together");
                // An unanswered prompt completes with PromptDismissed.
                return;
            }
            let dialog = AsyncFileDialog::new();
            let paths = match (options.directories, options.multiple) {
                (true, true) => dialog.pick_folders().await,
                (false, true) => dialog.pick_files().await,
                (true, false) => dialog.pick_folder().await.map(|path| vec![path]),
                (false, false) => dialog.pick_file().await.map(|path| vec![path]),
            };
            answer.send(
                paths.map(|paths| paths.into_iter().map(|file| file.path().into()).collect()),
            );
        }
        PathPrompt::Save { directory, answer } => {
            let path = AsyncFileDialog::new()
                .set_directory(directory)
                .save_file()
                .await;
            answer.send(path.map(|file| file.path().into()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::prompt::{PathPromptOptions, PromptDismissed};
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    #[test]
    fn unsupported_mixed_prompt_completes_without_opening_a_dialog() {
        let mut app = AppContext::new();
        let showing = Rc::default();
        let mut reply = pin!(app.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: true,
        }));
        let mut cx = Context::from_waker(Waker::noop());
        app.poll_foreground_tasks();
        assert!(matches!(reply.as_mut().poll(&mut cx), Poll::Pending));

        run_path_prompts(&mut app, &showing);
        app.poll_foreground_tasks();
        app.poll_foreground_tasks();

        assert!(matches!(
            reply.as_mut().poll(&mut cx),
            Poll::Ready(Err(PromptDismissed))
        ));
        assert!(!showing.get());
    }
}
