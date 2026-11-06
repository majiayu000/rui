use rui::advanced_ui::Button;
use rui::core::prompt::{PathPrompt, PathPromptOptions, PromptDismissed};
use rui::core::{AppContext, Point, Size, Task, View, ViewContext};
use rui::elements::{Element, div};
use rui::testing::{HeadlessSession, mount, mount_view};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn mount_or_panic<F, E>(build_root: F) -> HeadlessSession<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    match mount(Size::new(200.0, 100.0), build_root) {
        Ok(session) => session,
        Err(err) => panic!("headless prompt session should mount: {err}"),
    }
}

/// Await `task` in a spawned task and keep its output.
fn collect<T: 'static>(context: &mut AppContext, task: Task<T>) -> Rc<RefCell<Option<T>>> {
    let output = Rc::new(RefCell::new(None));
    let slot = Rc::clone(&output);
    context
        .spawn(move |_| async move {
            *slot.borrow_mut() = Some(task.await);
        })
        .detach();
    output
}

fn only_prompt<F, E>(session: &mut HeadlessSession<F, E>) -> PathPrompt
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    let mut prompts = session.take_path_prompts();
    assert_eq!(prompts.len(), 1, "exactly one prompt should be waiting");
    prompts.remove(0)
}

#[test]
fn an_open_prompt_finishes_with_the_paths_the_user_chose() {
    let mut session = mount_or_panic(|_| div());
    let options = PathPromptOptions {
        files: true,
        directories: true,
        multiple: true,
    };
    let task = session.app_context_mut().prompt_for_paths(options);
    let chosen = collect(session.app_context_mut(), task);
    session.run_until_parked();
    assert!(chosen.borrow().is_none(), "the task waits for the answer");

    match only_prompt(&mut session) {
        PathPrompt::Open {
            options: asked,
            answer,
        } => {
            assert_eq!(asked, options);
            answer.send(Some(vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")]));
        }
        other => panic!("expected an open prompt, got {other:?}"),
    }
    session.run_until_parked();

    assert_eq!(
        chosen.borrow_mut().take(),
        Some(Ok(Some(vec![
            PathBuf::from("/tmp/a"),
            PathBuf::from("/tmp/b")
        ])))
    );
    assert!(session.take_path_prompts().is_empty());
}

#[test]
fn a_cancelled_prompt_finishes_with_none_and_a_dropped_one_with_an_error() {
    let mut session = mount_or_panic(|_| div());
    let cancelled = session
        .app_context_mut()
        .prompt_for_paths(PathPromptOptions::default());
    let cancelled = collect(session.app_context_mut(), cancelled);
    let dropped = session
        .app_context_mut()
        .prompt_for_new_path(Path::new("/tmp"));
    let dropped = collect(session.app_context_mut(), dropped);

    let mut prompts = session.take_path_prompts().into_iter();
    match prompts.next() {
        Some(PathPrompt::Open { answer, .. }) => answer.send(None),
        other => panic!("expected the open prompt first, got {other:?}"),
    }
    // A platform that cannot show the save panel drops it.
    drop(prompts);
    session.run_until_parked();

    assert_eq!(cancelled.borrow_mut().take(), Some(Ok(None)));
    assert_eq!(dropped.borrow_mut().take(), Some(Err(PromptDismissed)));
}

#[test]
fn a_save_prompt_starts_in_its_directory_and_finishes_with_the_new_path() {
    let mut session = mount_or_panic(|_| div());
    let task = session
        .app_context_mut()
        .prompt_for_new_path(Path::new("/tmp/notes"));
    let saved = collect(session.app_context_mut(), task);

    match only_prompt(&mut session) {
        PathPrompt::Save { directory, answer } => {
            assert_eq!(directory, PathBuf::from("/tmp/notes"));
            answer.send(Some(directory.join("todo.txt")));
        }
        other => panic!("expected a save prompt, got {other:?}"),
    }
    session.run_until_parked();

    assert_eq!(
        saved.borrow_mut().take(),
        Some(Ok(Some(PathBuf::from("/tmp/notes/todo.txt"))))
    );
}

#[test]
#[should_panic(expected = "files or directories")]
fn a_prompt_that_allows_nothing_is_rejected() {
    let mut session = mount_or_panic(|_| div());
    let _task = session
        .app_context_mut()
        .prompt_for_paths(PathPromptOptions {
            files: false,
            directories: false,
            multiple: false,
        });
}

/// A view that opens files from a button and lists what it opened.
struct Opener {
    opened: Rc<RefCell<Vec<PathBuf>>>,
}

impl View for Opener {
    type Element = Button;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let opened = Rc::clone(&self.opened);
        Button::new("Open").on_click(cx.listener(move |_, cx| {
            let paths = cx.prompt_for_paths(PathPromptOptions::default());
            let opened = Rc::clone(&opened);
            cx.app_mut()
                .spawn(move |_| async move {
                    if let Ok(Some(paths)) = paths.await {
                        opened.borrow_mut().extend(paths);
                    }
                })
                .detach();
        }))
    }
}

#[test]
fn a_view_listener_prompts_and_its_task_gets_the_answer() {
    let opened = Rc::new(RefCell::new(Vec::new()));
    let view = Opener {
        opened: Rc::clone(&opened),
    };
    let mut session = match mount_view(Size::new(160.0, 80.0), view) {
        Ok(session) => session,
        Err(err) => panic!("opener view should mount: {err}"),
    };
    assert!(session.pointer_down(Point::new(4.0, 4.0)));
    assert!(session.pointer_up(Point::new(4.0, 4.0)));
    if let Err(err) = session.frame() {
        panic!("frame after the click failed: {err}");
    }

    match only_prompt(&mut session) {
        PathPrompt::Open { answer, .. } => answer.send(Some(vec![PathBuf::from("/tmp/readme.md")])),
        other => panic!("expected an open prompt, got {other:?}"),
    }
    session.run_until_parked();
    assert_eq!(
        opened.borrow().as_slice(),
        [PathBuf::from("/tmp/readme.md")]
    );
}
