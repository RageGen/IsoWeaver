mod backend;
mod runtime;

use backend::{Change, inspect_boot, save_image};
use gtk::{
    gio, glib, prelude::*, Application, ApplicationWindow, Box as GtkBox, Button, FileChooserAction,
    FileChooserNative, Label, ListBox, ListBoxRow, Orientation, Paned, ProgressBar, ScrolledWindow,
    SelectionMode,
};
use std::{
    cell::RefCell,
    io::Write,
    path::{Component, Path, PathBuf},
    process::Stdio,
    rc::Rc,
};

#[derive(Clone, Debug)]
struct Entry {
    path: String,
    directory: bool,
    size: u64,
    source: Option<PathBuf>,
}

fn safe_image_path(raw: &str) -> Result<PathBuf, String> {
    let path = Path::new(raw);
    if path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!("Unsafe image path: {raw}"));
    }
    Ok(path.to_path_buf())
}

fn list_image(path: &Path) -> Result<Vec<Entry>, String> {
    let output = runtime::command("7z")
        .args(["l", "-slt", "--"])
        .arg(path)
        .output()
        .map_err(|error| format!("Could not start 7z: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }

    let mut entries = Vec::new();
    let mut fields = std::collections::HashMap::<String, String>::new();
    let archive_name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    for line in String::from_utf8_lossy(&output.stdout)
        .lines()
        .chain(std::iter::once(""))
    {
        if line.trim().is_empty() {
            if let Some(name) = fields.remove("Path") {
                if name != archive_name && fields.contains_key("Folder") {
                    let relative = safe_image_path(&name)?;
                    if fields
                        .get("Attributes")
                        .is_some_and(|attributes| attributes.to_ascii_lowercase().contains('l'))
                    {
                        return Err(format!("Symbolic links are not supported: {name}"));
                    }
                    entries.push(Entry {
                        source: None,
                        path: relative.to_string_lossy().into_owned(),
                        directory: fields.get("Folder").is_some_and(|value| value == "+"),
                        size: fields
                            .get("Size")
                            .and_then(|value| value.parse().ok())
                            .unwrap_or(0),
                    });
                }
            }
            fields.clear();
        } else if let Some((key, value)) = line.split_once(" = ") {
            fields.insert(key.to_owned(), value.to_owned());
        }
    }
    Ok(entries)
}

fn main() -> glib::ExitCode {
    gio::resources_register_include!("icons.gresource").expect("Embedded icons must load");
    let app = Application::builder()
        .application_id("org.isoweaver.IsoWeaver")
        .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    if app.active_window().is_some() {
        return;
    }

    gtk::Window::set_default_icon_name("org.isoweaver.IsoWeaver");
    install_theme();
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::IconTheme::for_display(&display).add_resource_path("/org/isoweaver/IsoWeaver/icons");
    }
    let window = ApplicationWindow::builder()
        .application(app)
        .title("IsoWeaver")
        .default_width(1240)
        .default_height(800)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 0);
    let header = gtk::HeaderBar::new();
    let app_title = Label::new(Some("IsoWeaver"));
    app_title.add_css_class("title");
    header.set_title_widget(Some(&app_title));
    window.set_titlebar(Some(&header));

    let open = action_button("Open", "document-open-symbolic", "action-open", "Open an ISO image");
    let save = action_button("Save", "document-save-symbolic", "action-save", "Save a dated edited copy beside the source image");
    let add_files = action_button("Add Files", "list-add-symbolic", "action-add", "Add host files to the current image folder");
    let add_directory = action_button("Add Folder", "folder-new-symbolic", "action-add", "Add a host folder to the image");
    let extract = action_button("Extract Selected", "go-up-symbolic", "action-extract", "Extract selected entries to your computer");
    let extract_all = action_button("Extract All", "go-up-symbolic", "action-extract", "Extract every entry to your computer");
    let delete = action_button("Delete Selected", "edit-delete-symbolic", "action-delete", "Delete selected entries from the image");
    let rename = action_button("Rename Selected", "document-edit-symbolic", "action-edit", "Rename one selected entry");
    let create_directory = action_button("New Folder in Image", "folder-new-symbolic", "action-edit", "Create an empty folder inside the image");
    let image_info = Button::with_label("Image Information");
    image_info.add_css_class("flat");
    let search_action = Button::with_label("Search image contents");
    search_action.add_css_class("flat");
    let about_action = Button::with_label("About IsoWeaver");
    about_action.add_css_class("flat");

    let edit_menu = [("_Rename Selected", &rename), ("_New Folder in Image", &create_directory)];
    let view_menu = [("_Search Image Contents", &search_action)];
    let tools_menu = [("Image _Information", &image_info)];
    let help_menu = [("_About IsoWeaver", &about_action)];
    let menu_panel = GtkBox::new(Orientation::Horizontal, 2);
    menu_panel.set_margin_start(12);
    menu_panel.set_margin_end(12);
    menu_panel.add_css_class("menu-panel");
    menu_panel.append(&dropdown_menu("Edit", &edit_menu));
    menu_panel.append(&dropdown_menu("View", &view_menu));
    menu_panel.append(&dropdown_menu("Tools", &tools_menu));
    menu_panel.append(&dropdown_menu("Help", &help_menu));
    root.append(&menu_panel);

    let toolbar = GtkBox::new(Orientation::Horizontal, 6);
    toolbar.set_margin_start(16);
    toolbar.set_margin_end(16);
    toolbar.set_margin_top(10);
    toolbar.set_margin_bottom(10);
    toolbar.add_css_class("toolbar-row");
    for button in [&open, &save] { toolbar.append(button); }
    toolbar.append(&gtk::Separator::new(Orientation::Vertical));
    for button in [&add_files, &add_directory] { toolbar.append(button); }
    toolbar.append(&gtk::Separator::new(Orientation::Vertical));
    for button in [&extract, &extract_all] { toolbar.append(button); }
    toolbar.append(&gtk::Separator::new(Orientation::Vertical));
    toolbar.append(&delete);
    root.append(&toolbar);

    let current_image_label = Label::new(Some("No image open"));
    current_image_label.set_xalign(0.0);
    current_image_label.add_css_class("image-title");
    let image_state_label = Label::new(Some("Ready to open an image"));
    image_state_label.set_xalign(1.0);
    image_state_label.add_css_class("status-badge");
    image_state_label.add_css_class("status-text");
    let session_banner = GtkBox::new(Orientation::Horizontal, 12);
    session_banner.set_margin_start(16);
    session_banner.set_margin_end(16);
    session_banner.set_margin_bottom(12);
    session_banner.add_css_class("session-banner");
    session_banner.append(&current_image_label);
    let banner_spacer = GtkBox::new(Orientation::Horizontal, 0);
    banner_spacer.set_hexpand(true);
    session_banner.append(&banner_spacer);
    session_banner.append(&image_state_label);
    root.append(&session_banner);

    let tree = ListBox::new();
    tree.set_selection_mode(SelectionMode::Single);
    tree.add_css_class("directory-tree");
    let tree_scroll = ScrolledWindow::builder()
        .min_content_width(220)
        .hexpand(true)
        .vexpand(true)
        .child(&tree)
        .build();
    let sidebar = GtkBox::new(Orientation::Vertical, 8);
    sidebar.set_margin_start(0);
    sidebar.set_margin_end(0);
    sidebar.set_margin_top(12);
    sidebar.add_css_class("isoweaver-sidebar");
    let tree_heading = Label::new(Some("Folders"));
    tree_heading.set_xalign(0.0);
    tree_heading.set_margin_start(16);
    tree_heading.add_css_class("section-heading");
    sidebar.append(&tree_heading);
    sidebar.append(&tree_scroll);

    let file_list = ListBox::new();
    file_list.set_selection_mode(SelectionMode::Multiple);
    file_list.set_tooltip_text(Some("Click to select; Ctrl-click to add or remove files from the selection"));
    file_list.add_css_class("file-list");
    let click = gtk::GestureClick::new();
    click.set_button(1);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let list_for_click = file_list.clone();
    click.connect_pressed(move |gesture, presses, _, y| {
        if let Some(row) = list_for_click.row_at_y(y.floor() as i32) {
            let control = gesture.current_event_state().contains(gtk::gdk::ModifierType::CONTROL_MASK);
            select_clicked_row(&list_for_click, &row, control);
            row.grab_focus();
            let _ = gesture.set_state(gtk::EventSequenceState::Claimed);
            if presses == 2 && !control {
                row.emit_activate();
            }
        }
    });
    file_list.add_controller(click);
    let file_scroll = ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&file_list)
        .build();
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search image contents"));
    let breadcrumb = Label::new(Some("Image: /"));
    breadcrumb.set_xalign(0.0);
    breadcrumb.add_css_class("dim-label");
    let file_top = GtkBox::new(Orientation::Horizontal, 10);
    file_top.set_margin_bottom(8);
    file_top.append(&breadcrumb);
    let top_spacer = GtkBox::new(Orientation::Horizontal, 0);
    top_spacer.set_hexpand(true);
    file_top.append(&top_spacer);
    file_top.append(&search);

    let file_header = GtkBox::new(Orientation::Horizontal, 12);
    file_header.set_margin_start(14);
    file_header.set_margin_end(14);
    file_header.set_margin_top(8);
    file_header.set_margin_bottom(8);
    file_header.add_css_class("file-list-header");
    let name_heading = Label::new(Some("Name"));
    name_heading.set_xalign(0.0);
    name_heading.set_hexpand(true);
    let size_heading = Label::new(Some("Size"));
    size_heading.set_width_chars(12);
    size_heading.set_xalign(1.0);
    let type_heading = Label::new(Some("Type"));
    type_heading.set_width_chars(14);
    type_heading.set_xalign(0.0);
    for heading in [&name_heading, &size_heading, &type_heading] {
        heading.add_css_class("column-heading");
        file_header.append(heading);
    }
    let browser = GtkBox::new(Orientation::Vertical, 0);
    browser.set_margin_start(8);
    browser.set_margin_end(12);
    browser.set_margin_top(12);
    browser.append(&file_top);
    browser.append(&file_header);
    browser.append(&file_scroll);

    let paned = Paned::new(Orientation::Horizontal);
    paned.set_position(270);
    paned.set_wide_handle(true);
    paned.set_start_child(Some(&sidebar));
    paned.set_end_child(Some(&browser));
    paned.set_vexpand(true);
    root.append(&paned);

    let status = Label::new(Some("Ready · Open an ISO image to begin"));
    status.set_xalign(0.0);
    status.add_css_class("status-text");
    let selection_status = status.clone();
    file_list.connect_selected_rows_changed(move |list| {
        let count = list.selected_rows().len();
        if count > 0 {
            selection_status.set_text(&format!("{count} selected · Ctrl-click to add or remove"));
        }
    });
    status.set_hexpand(true);
    let operation_panel = GtkBox::new(Orientation::Horizontal, 12);
    operation_panel.set_margin_start(16);
    operation_panel.set_margin_end(16);
    operation_panel.set_margin_top(10);
    operation_panel.set_margin_bottom(12);
    operation_panel.add_css_class("operation-panel");
    operation_panel.append(&status);
    root.append(&operation_panel);
    window.set_child(Some(&root));

    let current: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let all_entries: Rc<RefCell<Vec<Entry>>> = Rc::new(RefCell::new(Vec::new()));
    let visible_entries: Rc<RefCell<Vec<Entry>>> = Rc::new(RefCell::new(Vec::new()));
    let current_dir: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let tree_paths: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let changes: Rc<RefCell<Vec<Change>>> = Rc::new(RefCell::new(Vec::new()));
    let current_state = current.clone();
    let window_state = window.clone();
    let tree_widget = tree.clone();
    let tree_paths_state = tree_paths.clone();
    let breadcrumb_widget = breadcrumb.clone();

    let current_clone = current.clone();
    let all_clone = all_entries.clone();
    let visible_clone = visible_entries.clone();
    let dir_clone = current_dir.clone();
    let list_for_tree = file_list.clone();
    let breadcrumb_for_tree = breadcrumb.clone();
    let paths_clone = tree_paths.clone();
    let search_clone = search.clone();
    let status_clone = status.clone();
    tree.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            if let Some(path) = paths_clone.borrow().get(row.index() as usize).cloned() {
                *dir_clone.borrow_mut() = path;
                render_files(
                    &list_for_tree,
                    &all_clone.borrow(),
                    &visible_clone,
                    &dir_clone.borrow(),
                    &search_clone.text(),
                    &breadcrumb_for_tree,
                );
                if current_clone.borrow().is_some() {
                    status_clone.set_text("Ready · Select entries to extract or edit");
                }
            }
        }
    });

    let current_clone = current.clone();
    let all_clone = all_entries.clone();
    let visible_clone = visible_entries.clone();
    let dir_clone = current_dir.clone();
    let tree_clone = tree.clone();
    let tree_paths_clone = tree_paths.clone();
    let search_clone = search.clone();
    let breadcrumb_clone = breadcrumb.clone();
    let file_list_for_activation = file_list.clone();
    file_list.clone().connect_row_activated(move |_, row| {
        let Some(entry) = visible_clone.borrow().get(row.index() as usize).cloned() else {
            return;
        };
        if entry.directory {
            *dir_clone.borrow_mut() = entry.path.clone();
            if let Some(index) = tree_paths_clone.borrow().iter().position(|path| path == &entry.path) {
                if let Some(tree_row) = tree_clone.row_at_index(index as i32) {
                    tree_clone.select_row(Some(&tree_row));
                }
            }
            render_files(
                &file_list_for_activation,
                &all_clone.borrow(),
                &visible_clone,
                &dir_clone.borrow(),
                &search_clone.text(),
                &breadcrumb_clone,
            );
        }
        let _ = current_clone.borrow();
    });

    let all_clone = all_entries.clone();
    let visible_clone = visible_entries.clone();
    let dir_clone = current_dir.clone();
    let list_clone = file_list.clone();
    let breadcrumb_clone = breadcrumb.clone();
    search.connect_search_changed(move |entry| {
        render_files(
            &list_clone,
            &all_clone.borrow(),
            &visible_clone,
            &dir_clone.borrow(),
            &entry.text(),
            &breadcrumb_clone,
        );
    });
    let search_focus = search.clone();
    search_action.connect_clicked(move |_| { search_focus.grab_focus(); });
    let about_parent = window_state.clone();
    about_action.connect_clicked(move |_| {
        let about = gtk::Dialog::builder()
            .transient_for(&about_parent).modal(true).title("About IsoWeaver")
            .default_width(420).resizable(false).build();
        let close = about.add_button("Close", gtk::ResponseType::Close);
        close.set_margin_start(12);
        close.set_margin_end(12);
        close.set_margin_top(8);
        close.set_margin_bottom(12);
        about.connect_response(|dialog, _| dialog.close());
        let content = GtkBox::new(Orientation::Vertical, 12);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        content.set_margin_start(24);
        content.set_margin_end(24);
        if let Ok(pixbuf) = gdk_pixbuf::Pixbuf::from_read(
            std::io::Cursor::new(include_bytes!("../data/icons/256x256/apps/org.isoweaver.IsoWeaver.png")),
        ) {
            let logo = gtk::Image::from_paintable(Some(&gtk::gdk::Texture::for_pixbuf(&pixbuf)));
            logo.set_pixel_size(96);
            content.append(&logo);
        }
        let title = Label::new(Some(&format!("IsoWeaver {}", env!("CARGO_PKG_VERSION"))));
        title.add_css_class("title-1");
        content.append(&title);
        let description = Label::new(Some("Browse, extract, and edit ISO images on Linux."));
        description.set_wrap(true);
        description.set_selectable(true);
        description.set_xalign(0.5);
        description.set_justify(gtk::Justification::Center);
        content.append(&description);
        content.append(&gtk::LinkButton::with_label(
            "https://www.gnu.org/licenses/gpl-3.0.html",
            "GNU GPL version 3 or later",
        ));
        about.content_area().append(&content);
        about.present();
    });

    let parent = window_state.clone();
    let current_open = current.clone();
    let all_open = all_entries.clone();
    let visible_open = visible_entries.clone();
    let dir_open = current_dir.clone();
    let pending_open = changes.clone();
    let open_label = current_image_label.clone();
    let open_state = image_state_label.clone();
    let open_status = status.clone();
    let open_tree = tree.clone();
    let open_tree_paths = tree_paths.clone();
    let open_file_list = file_list.clone();
    let open_breadcrumb = breadcrumb.clone();
    let open_search = search.clone();
    open.connect_clicked(move |_| {
        let parent = parent.clone();
        let current = current_open.clone();
        let all = all_open.clone();
        let visible = visible_open.clone();
        let dir = dir_open.clone();
        let pending = pending_open.clone();
        let title = open_label.clone();
        let image_state = open_state.clone();
        let status = open_status.clone();
        let tree = open_tree.clone();
        let tree_paths = open_tree_paths.clone();
        let list = open_file_list.clone();
        let breadcrumb = open_breadcrumb.clone();
        let search = open_search.clone();
        let parent_for_callback = parent.clone();
        choose_one(&parent, FileChooserAction::Open, "Open image", move |path| {
            let Some(path) = path else { return };
            let pending_for_check = pending.clone();
            let pending_for_load = pending.clone();
            let load = move || {
                pending_for_load.borrow_mut().clear();
                load_image(
                    &path, &list, &tree, &title, &image_state, &status, &current, &all,
                    &visible, &dir, &tree_paths, &breadcrumb, &search,
                );
            };
            if pending_for_check.borrow().is_empty() {
                load();
            } else {
                confirm(
                    &parent_for_callback,
                    "Discard pending changes?",
                    "Opening another image will discard this session's unsaved changes.",
                    move |yes| if yes { load(); },
                );
            }
        });
    });

    let parent = window_state.clone();
    let current = current_state.clone();
    let all = all_entries.clone();
    let dir = current_dir.clone();
    let pending = changes.clone();
    let preview = AddPreview {
        all: all_entries.clone(), visible: visible_entries.clone(), dir: current_dir.clone(),
        tree: tree.clone(), tree_paths: tree_paths.clone(), list: file_list.clone(),
        breadcrumb: breadcrumb.clone(), search: search.clone(),
    };
    let preview_files = preview.clone();
    let status_add = status.clone();
    let state_add = image_state_label.clone();
    add_files.connect_clicked(move |_| {
        let preview = preview_files.clone();
        if current.borrow().is_none() { return; }
        let parent = parent.clone();
        let all = all.clone();
        let dir = dir.borrow().clone();
        let pending = pending.clone();
        let status = status_add.clone();
        let image_state = state_add.clone();
        let parent_for_callback = parent.clone();
        file_dialog(&parent, move |files| {
            let candidates = map_sources(files, &dir, &status);
            let collision = candidates.iter().any(|(_, target)| {
                all.borrow().iter().any(|entry| format!("/{}", entry.path) == *target)
            });
            if collision {
                let pending = pending.clone();
                let status = status.clone();
                let image_state = image_state.clone();
                confirm(&parent_for_callback, "Replace image entries?", "Some selected names already exist in this folder. They will be replaced when saved.", move |yes| {
                    if yes { stage_adds(&candidates, &pending, &status, &image_state, &preview); }
                });
            } else {
                stage_adds(&candidates, &pending, &status, &image_state, &preview);
            }
        });
    });

    let parent = window_state.clone();
    let current = current_state.clone();
    let all = all_entries.clone();
    let dir = current_dir.clone();
    let pending = changes.clone();
    let status_add_dir = status.clone();
    let state_add_dir = image_state_label.clone();
    add_directory.connect_clicked(move |_| {
        let preview = preview.clone();
        if current.borrow().is_none() { return; }
        let parent = parent.clone();
        let dir = dir.borrow().clone();
        let all = all.clone();
        let pending = pending.clone();
        let status = status_add_dir.clone();
        let image_state = state_add_dir.clone();
        let parent_for_callback = parent.clone();
        choose_one(&parent, FileChooserAction::SelectFolder, "Choose a directory to add", move |source| {
            let Some(source) = source else { return };
            let candidates = map_sources(vec![source], &dir, &status);
            let collision = candidates.iter().any(|(_, target)| {
                all.borrow().iter().any(|entry| format!("/{}", entry.path) == *target)
            });
            if collision {
                let pending = pending.clone();
                let status = status.clone();
                let image_state = image_state.clone();
                confirm(&parent_for_callback, "Replace image directory?", "This directory already exists in the image and will be replaced when saved.", move |yes| {
                    if yes { stage_adds(&candidates, &pending, &status, &image_state, &preview); }
                });
            } else {
                stage_adds(&candidates, &pending, &status, &image_state, &preview);
            }
        });
    });

    let parent = window_state.clone();
    let current = current_state.clone();
    let visible = visible_entries.clone();
    let list = file_list.clone();
    let pending = changes.clone();
    let status_delete = status.clone();
    let state_delete = image_state_label.clone();
    delete.connect_clicked(move |_| {
        if current.borrow().is_none() { return; }
        let selected = selected_entries(&list, &visible);
        if selected.is_empty() { return; }
        let pending = pending.clone();
        let status = status_delete.clone();
        let image_state = state_delete.clone();
        confirm(&parent, "Delete from image?", "These entries will be removed when you save the modified image.", move |yes| {
            if yes {
                let mut changes = pending.borrow_mut();
                for entry in &selected {
                    let image_path = format!("/{}", entry.path);
                    if !changes.iter().any(|change| matches!(change, Change::Delete { image_path: existing, .. } if existing == &image_path)) {
                        changes.push(Change::Delete { image_path, directory: entry.directory });
                    }
                }
                status.set_text(&format!("Modified · {} pending changes", changes.len()));
                image_state.set_text("Modified (*) · Unsaved changes");
            }
        });
    });

    let parent = window_state.clone();
    let current = current_state.clone();
    let visible = visible_entries.clone();
    let list = file_list.clone();
    let dir = current_dir.clone();
    let pending = changes.clone();
    let status_rename = status.clone();
    let state_rename = image_state_label.clone();
    rename.connect_clicked(move |_| {
        if current.borrow().is_none() { return; }
        let selected = selected_entries(&list, &visible);
        if selected.len() != 1 { status_rename.set_text("Select one entry to rename"); return; }
        let entry = selected[0].clone();
        let pending = pending.clone();
        let status = status_rename.clone();
        let image_state = state_rename.clone();
        text_input(&parent, "Rename entry", "Enter the new name:", move |value| {
            let Some(name) = value else { return };
            if !valid_leaf(&name) { status.set_text("Enter a single name without slash or dot segments"); return; }
            let parent = Path::new(&entry.path).parent().unwrap_or(Path::new(""));
            let new_path = parent.join(name).to_string_lossy().into_owned();
            pending.borrow_mut().push(Change::Rename { old_path: format!("/{}", entry.path), new_path: format!("/{new_path}") });
            status.set_text(&format!("Modified · {} pending changes", pending.borrow().len()));
            image_state.set_text("Modified (*) · Unsaved changes");
        });
        let _ = dir.borrow();
    });

    let current = current_state.clone();
    let dir = current_dir.clone();
    let window_for_mkdir = window_state.clone();
    let pending = changes.clone();
    let status_mkdir = status.clone();
    let state_mkdir = image_state_label.clone();
    create_directory.connect_clicked(move |_| {
        if current.borrow().is_none() { return; }
        let base = dir.borrow().clone();
        let pending = pending.clone();
        let status = status_mkdir.clone();
        let image_state = state_mkdir.clone();
        text_input(&window_for_mkdir, "Create directory", "Enter a single directory name:", move |value| {
            let Some(name) = value else { return };
            if !valid_leaf(&name) { status.set_text("Enter a single name without slash or dot segments"); return; }
            let path = if base.is_empty() { name } else { format!("{base}/{name}") };
            pending.borrow_mut().push(Change::CreateDirectory { image_path: format!("/{path}") });
            status.set_text(&format!("Modified · {} pending changes", pending.borrow().len()));
            image_state.set_text("Modified (*) · Unsaved changes");
        });
    });

    let parent = window_state.clone();
    let current = current_state.clone();
    image_info.connect_clicked(move |_| {
        let Some(image) = current.borrow().clone() else { return };
        let parent = parent.clone();
        let source = image.clone();
        run_operation(&parent.clone(), "Reading image information…", move || inspect_boot(&source), move |result| match result {
            Ok(report) => image_information_dialog(&parent, &image, &report),
            Err(error) => error_dialog(&parent, &error),
        });
    });

    let parent = window_state.clone();
    let current = current_state.clone();
    let visible = visible_entries.clone();
    let all = all_entries.clone();
    let list = file_list.clone();
    extract.connect_clicked(move |_| {
        let Some(image) = current.borrow().clone() else { return };
        let selected = selected_entries(&list, &visible);
        if selected.is_empty() { return; }
        let expanded = expand_directories(&selected, &all.borrow());
        let parent = parent.clone();
        let parent_for_callback = parent.clone();
        choose_one(&parent, FileChooserAction::SelectFolder, "Choose extraction folder", move |destination| {
            if let Some(destination) = destination {
                run_operation(&parent_for_callback.clone(), "Extracting image contents…",
                    move || extract_entries(&image, &destination, &expanded),
                    move |result| if let Err(error) = result { error_dialog(&parent_for_callback, &error); });
            }
        });
    });

    let parent = window_state.clone();
    let current = current_state.clone();
    let all = all_entries.clone();
    extract_all.connect_clicked(move |_| {
        let Some(image) = current.borrow().clone() else { return };
        let entries = all.borrow().clone();
        let parent = parent.clone();
        let parent_for_callback = parent.clone();
        choose_one(&parent, FileChooserAction::SelectFolder, "Choose extraction folder", move |destination| {
            if let Some(destination) = destination {
                run_operation(&parent_for_callback.clone(), "Extracting image contents…",
                    move || extract_entries(&image, &destination, &entries),
                    move |result| if let Err(error) = result { error_dialog(&parent_for_callback, &error); });
            }
        });
    });

    let current = current_state.clone();
    let pending = changes.clone();
    let all = all_entries.clone();
    let visible = visible_entries.clone();
    let dir = current_dir.clone();
    let tree = tree_widget.clone();
    let tree_paths = tree_paths_state.clone();
    let title = current_image_label.clone();
    let image_state = image_state_label.clone();
    let status_save = status.clone();
    let breadcrumb = breadcrumb_widget.clone();
    let search_save = search.clone();
    let list = file_list.clone();
    let window_for_save = window_state.clone();
    save.connect_clicked(move |_| {
        let Some(image) = current.borrow().clone() else { return };
        let staged = pending.borrow().clone();
        if staged.is_empty() { return; }
        let state = SaveUi { tree: tree.clone(), title: title.clone(), image_state: image_state.clone(), status: status_save.clone(), list: list.clone(), all: all.clone(), visible: visible.clone(), current: current.clone(), dir: dir.clone(), tree_paths: tree_paths.clone(), breadcrumb: breadcrumb.clone(), search: search_save.clone(), pending: pending.clone() };
        let parent = window_for_save.clone();
        match dated_image_path(&image) {
            Ok(target) => finish_save(&parent, &image, &target, &staged, false, &state),
            Err(error) => error_dialog(&parent, &error),
        }
    });

    let pending_close = changes.clone();
    let close_parent = window_state.clone();
    window_state.connect_close_request(move |_| {
        if pending_close.borrow().is_empty() { return glib::Propagation::Proceed; }
        let pending = pending_close.clone();
        let close_parent_inner = close_parent.clone();
        confirm(&close_parent, "Discard unsaved changes?", "There are image changes that have not been saved.", move |yes| {
            if yes { pending.borrow_mut().clear(); close_parent_inner.close(); }
        });
        glib::Propagation::Stop
    });

    window.present();
}

#[derive(Clone)]
struct SaveUi {
    tree: ListBox,
    title: Label,
    image_state: Label,
    status: Label,
    list: ListBox,
    all: Rc<RefCell<Vec<Entry>>>,
    visible: Rc<RefCell<Vec<Entry>>>,
    current: Rc<RefCell<Option<PathBuf>>>,
    dir: Rc<RefCell<String>>,
    tree_paths: Rc<RefCell<Vec<String>>>,
    breadcrumb: Label,
    search: gtk::SearchEntry,
    pending: Rc<RefCell<Vec<Change>>>,
}

fn load_image(
    path: &Path,
    file_list: &ListBox,
    tree: &ListBox,
    title: &Label,
    image_state: &Label,
    status: &Label,
    current: &Rc<RefCell<Option<PathBuf>>>,
    all: &Rc<RefCell<Vec<Entry>>>,
    visible: &Rc<RefCell<Vec<Entry>>>,
    dir: &Rc<RefCell<String>>,
    tree_paths: &Rc<RefCell<Vec<String>>>,
    breadcrumb: &Label,
    search: &gtk::SearchEntry,
) {
    let parent = file_list.root().and_downcast::<ApplicationWindow>().expect("Image browser window");
    let file_list = file_list.clone();
    let tree = tree.clone();
    let title = title.clone();
    let image_state = image_state.clone();
    let status = status.clone();
    let current = current.clone();
    let all = all.clone();
    let visible = visible.clone();
    let dir = dir.clone();
    let tree_paths = tree_paths.clone();
    let breadcrumb = breadcrumb.clone();
    let search = search.clone();
    let path = path.to_path_buf();
    let source = path.clone();
    run_operation(&parent, "Opening image…", move || list_image(&source), move |result| {
    match result {
        Ok(mut entries) => {
            entries.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
            *current.borrow_mut() = Some(path.to_path_buf());
            *all.borrow_mut() = entries;
            *dir.borrow_mut() = String::new();
            title.set_text(path.file_name().and_then(|name| name.to_str()).unwrap_or("Image"));
            image_state.set_text("ISO image · Clean");
            render_tree(&tree, &all.borrow(), &tree_paths);
            render_files(&file_list, &all.borrow(), &visible, &dir.borrow(), &search.text(), &breadcrumb);
            status.set_text(&format!("{} entries · Select files to extract or edit", all.borrow().len()));
        }
        Err(error) => {
            *current.borrow_mut() = None;
            all.borrow_mut().clear();
            visible.borrow_mut().clear();
            clear_list(&file_list);
            clear_list(&tree);
            tree_paths.borrow_mut().clear();
            title.set_text("No image open");
            image_state.set_text("Ready to open an image");
            breadcrumb.set_text("Image: /");
            status.set_text(&format!("Could not open image: {error}"));
        }
    }
    });
}

fn render_tree(tree: &ListBox, entries: &[Entry], paths: &Rc<RefCell<Vec<String>>>) {
    clear_list(tree);
    let mut directories = entries.iter().filter(|entry| entry.directory).map(|entry| entry.path.clone()).collect::<Vec<_>>();
    directories.sort_by_key(|path| (path.matches('/').count(), path.to_lowercase()));
    let mut tree_items = vec![String::new()];
    tree_items.extend(directories);
    for path in &tree_items {
        let row = ListBoxRow::new();
        let content = GtkBox::new(Orientation::Horizontal, 8);
        content.set_margin_start(8 + (path.matches('/').count() as i32 * 12));
        content.set_margin_end(8);
        content.set_margin_top(6);
        content.set_margin_bottom(6);
        let icon = gtk::Image::from_icon_name(if path.is_empty() { "drive-harddisk-symbolic" } else { "folder-symbolic" });
        icon.set_pixel_size(16);
        let display = if path.is_empty() { "Image root" } else { path.rsplit('/').next().unwrap_or(path) };
        let name = Label::new(Some(display));
        name.set_xalign(0.0);
        name.set_hexpand(true);
        content.append(&icon);
        content.append(&name);
        row.set_child(Some(&content));
        tree.append(&row);
    }
    *paths.borrow_mut() = tree_items;
    if let Some(root) = tree.row_at_index(0) { tree.select_row(Some(&root)); }
}

fn render_files(
    list: &ListBox,
    all: &[Entry],
    visible: &Rc<RefCell<Vec<Entry>>>,
    directory: &str,
    query: &str,
    breadcrumb: &Label,
) {
    clear_list(list);
    let mut rows = all
        .iter()
        .filter(|entry| parent_directory(&entry.path) == directory)
        .filter(|entry| query.is_empty() || entry.path.to_lowercase().contains(&query.to_lowercase()))
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.directory.cmp(&a.directory).then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase())));
    for entry in &rows {
        let row = ListBoxRow::new();
        let content = GtkBox::new(Orientation::Horizontal, 12);
        content.set_margin_start(14);
        content.set_margin_end(14);
        content.set_margin_top(8);
        content.set_margin_bottom(8);
        let icon = gtk::Image::from_icon_name(if entry.directory { "folder-symbolic" } else { "text-x-generic-symbolic" });
        icon.set_pixel_size(18);
        let name = Label::new(Some(entry.path.rsplit('/').next().unwrap_or(&entry.path)));
        name.set_xalign(0.0);
        name.set_hexpand(true);
        name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        let size = Label::new(Some(if entry.directory { "—".into() } else { format_size(entry.size) }.as_str()));
        size.set_width_chars(12);
        size.set_xalign(1.0);
        size.add_css_class("dim-label");
        let kind = Label::new(Some(if entry.source.is_some() { "Added (unsaved)" } else if entry.directory { "Directory" } else { file_type(&entry.path) }));
        kind.set_width_chars(14);
        kind.set_xalign(0.0);
        kind.add_css_class("dim-label");
        content.append(&icon);
        content.append(&name);
        content.append(&size);
        content.append(&kind);
        row.set_child(Some(&content));
        row.set_tooltip_text(Some(&entry.path));
        list.append(&row);
    }
    *visible.borrow_mut() = rows;
    let crumb = if directory.is_empty() { "Image: /".to_owned() } else { format!("Image: /{directory}") };
    breadcrumb.set_text(&crumb);
}

fn parent_directory(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

fn file_type(path: &str) -> &str {
    Path::new(path).extension().and_then(|extension| extension.to_str()).unwrap_or("File")
}

fn selected_entries(list: &ListBox, entries: &Rc<RefCell<Vec<Entry>>>) -> Vec<Entry> {
    list.selected_rows().iter().filter_map(|row| entries.borrow().get(row.index() as usize).cloned()).collect()
}

fn expand_directories(selected: &[Entry], all: &[Entry]) -> Vec<Entry> {
    let mut result = Vec::new();
    let mut included_paths = std::collections::HashSet::new();
    for chosen in selected {
        if chosen.directory {
            let prefix = format!("{}/", chosen.path);
            for entry in all.iter().filter(|entry| entry.path == chosen.path || entry.path.starts_with(&prefix)) {
                if included_paths.insert(entry.path.clone()) {
                    result.push(entry.clone());
                }
            }
        } else if included_paths.insert(chosen.path.clone()) {
            result.push(chosen.clone());
        }
    }
    result
}

fn map_sources(sources: Vec<PathBuf>, directory: &str, status: &Label) -> Vec<(PathBuf, String)> {
    let mut mapped = Vec::new();
    for source in sources {
        let Some(name) = source.file_name().and_then(|name| name.to_str()).map(str::to_owned) else {
            status.set_text("Could not use a source with a non-Unicode name");
            continue;
        };
        let path = if directory.is_empty() { format!("/{name}") } else { format!("/{directory}/{name}") };
        mapped.push((source, path));
    }
    mapped
}

#[derive(Clone)]
struct AddPreview {
    all: Rc<RefCell<Vec<Entry>>>,
    visible: Rc<RefCell<Vec<Entry>>>,
    dir: Rc<RefCell<String>>,
    tree: ListBox,
    tree_paths: Rc<RefCell<Vec<String>>>,
    list: ListBox,
    breadcrumb: Label,
    search: gtk::SearchEntry,
}

fn collect_added_entries(source: &Path, image_path: &str) -> Result<Vec<Entry>, String> {
    let metadata = std::fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
        return Err(format!("Unsupported added file: {}", source.display()));
    }
    let mut entries = vec![Entry { path: image_path.trim_start_matches('/').into(),
        directory: metadata.is_dir(), size: metadata.len(), source: Some(source.to_path_buf()) }];
    if metadata.is_dir() {
        for child in std::fs::read_dir(source).map_err(|error| error.to_string())? {
            let child = child.map_err(|error| error.to_string())?;
            let name = child.file_name().into_string().map_err(|_| "Non-Unicode filename".to_owned())?;
            entries.extend(collect_added_entries(&child.path(), &format!("{image_path}/{name}"))?);
        }
    }
    Ok(entries)
}

fn merge_added_entries(all: &mut Vec<Entry>, additions: Vec<Entry>) {
    for entry in additions {
        if let Some(existing) = all.iter_mut().find(|existing| existing.path == entry.path) {
            *existing = entry;
        } else { all.push(entry); }
    }
}

fn stage_adds(candidates: &[(PathBuf, String)], pending: &Rc<RefCell<Vec<Change>>>, status: &Label, image_state: &Label, preview: &AddPreview) {
    let additions = match candidates.iter().try_fold(Vec::new(), |mut entries, (source, path)| {
        entries.extend(collect_added_entries(source, path)?);
        Ok::<_, String>(entries)
    }) {
        Ok(entries) => entries,
        Err(error) => { status.set_text(&format!("Could not add files: {error}")); return; }
    };
    let count = {
        let mut changes = pending.borrow_mut();
        for (source, image_path) in candidates {
            if let Some(change) = changes.iter_mut().find(|change| matches!(change, Change::Add { image_path: existing, .. } if existing == image_path)) {
                *change = Change::Add { source: source.clone(), image_path: image_path.clone() };
            } else {
                changes.push(Change::Add { source: source.clone(), image_path: image_path.clone() });
            }
        }
        changes.len()
    };
    merge_added_entries(&mut preview.all.borrow_mut(), additions);
    let directory = preview.dir.borrow().clone();
    // Clear active filters so successful additions are immediately visible.
    preview.search.set_text("");
    render_tree(&preview.tree, &preview.all.borrow(), &preview.tree_paths);
    let index = preview.tree_paths.borrow().iter().position(|path| path == &directory);
    if let Some(row) = index.and_then(|index| preview.tree.row_at_index(index as i32)) {
        preview.tree.select_row(Some(&row));
    }
    *preview.dir.borrow_mut() = directory.clone();
    render_files(&preview.list, &preview.all.borrow(), &preview.visible, &directory, "", &preview.breadcrumb);
    status.set_text(&format!("Added to editor · {count} pending changes"));
    image_state.set_text("Modified (*) · Unsaved changes");
}

fn finish_save(parent: &ApplicationWindow, input: &Path, target: &Path, changes: &[Change], replace: bool, ui: &SaveUi) {
    ui.status.set_text("Rebuilding and validating image…");
    let input = input.to_path_buf();
    let target = target.to_path_buf();
    let output = target.clone();
    let changes = changes.to_vec();
    let ui = ui.clone();
    let parent = parent.clone();
    run_operation(&parent.clone(), "Saving and validating edited image…",
        move || save_image(&input, &output, &changes, replace), move |result| {
    match result {
        Ok(()) => {
            ui.pending.borrow_mut().clear();
            load_image(&target, &ui.list, &ui.tree, &ui.title, &ui.image_state, &ui.status, &ui.current, &ui.all, &ui.visible, &ui.dir, &ui.tree_paths, &ui.breadcrumb, &ui.search);
            ui.image_state.set_text("ISO image · Clean");
            ui.status.set_text("Saved · rebuilt image reopened successfully");
            let _ = parent;
        }
        Err(error) => {
            ui.image_state.set_text("Modified · Save failed");
            ui.status.set_text("Save failed · original image preserved");
            error_dialog(&parent, &error);
        }
    }
    });
}

fn extract_entries(image: &Path, destination: &Path, entries: &[Entry]) -> Result<(), String> {
    let root = destination.canonicalize().map_err(|error| error.to_string())?;
    for entry in entries {
        let relative = safe_image_path(&entry.path)?;
        let target = root.join(&relative);
        let mut parent = root.clone();
        if let Some(parts) = relative.parent() {
            for component in parts.components() {
                if let Component::Normal(name) = component {
                    parent.push(name);
                    match std::fs::symlink_metadata(&parent) {
                        Ok(metadata) if metadata.file_type().is_symlink() => return Err(format!("Refusing to follow destination symlink: {}", parent.display())),
                        Ok(metadata) if !metadata.is_dir() => return Err(format!("Destination component is not a directory: {}", parent.display())),
                        Ok(_) => (),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => std::fs::create_dir(&parent).map_err(|error| error.to_string())?,
                        Err(error) => return Err(error.to_string()),
                    }
                }
            }
        }
        if !parent.canonicalize().map_err(|error| error.to_string())?.starts_with(&root) {
            return Err(format!("Extraction path escaped destination: {}", entry.path));
        }
        if entry.directory {
            match std::fs::symlink_metadata(&target) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => return Err(format!("Unsafe directory destination: {}", target.display())),
                Ok(_) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => std::fs::create_dir(&target).map_err(|error| error.to_string())?,
                Err(error) => return Err(error.to_string()),
            }
            continue;
        }
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&target).map_err(|error| format!("Cannot create {}: {error}", target.display()))?;
        if let Some(source) = &entry.source {
            let mut input = std::fs::File::open(source).map_err(|error| error.to_string())?;
            std::io::copy(&mut input, &mut file).map_err(|error| error.to_string())?;
            continue;
        }
        let mut child = runtime::command("7z").arg("x").arg("-so").arg("--").arg(image).arg(&entry.path).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|error| error.to_string())?;
        if let Some(mut stdout) = child.stdout.take() { std::io::copy(&mut stdout, &mut file).map_err(|error| error.to_string())?; }
        file.flush().map_err(|error| error.to_string())?;
        let result = child.wait_with_output().map_err(|error| error.to_string())?;
        if !result.status.success() {
            let _ = std::fs::remove_file(&target);
            return Err(format!("Could not extract {}: {}", entry.path, String::from_utf8_lossy(&result.stderr)));
        }
    }
    Ok(())
}

fn choose_one(parent: &ApplicationWindow, action: FileChooserAction, title: &str, callback: impl FnOnce(Option<PathBuf>) + 'static) {
    let chooser = FileChooserNative::new(Some(title), Some(parent), action, Some("Select"), Some("Cancel"));
    let callback = RefCell::new(Some(callback));
    chooser.connect_response(move |dialog, response| {
        let path = if response == gtk::ResponseType::Accept { dialog.file().and_then(|file| file.path()) } else { None };
        if let Some(callback) = callback.borrow_mut().take() { callback(path); }
        dialog.destroy();
    });
    chooser.show();
}

fn file_dialog(parent: &ApplicationWindow, callback: impl FnOnce(Vec<PathBuf>) + 'static) {
    let chooser = FileChooserNative::new(Some("Add files to image"), Some(parent), FileChooserAction::Open, Some("Add"), Some("Cancel"));
    chooser.set_select_multiple(true);
    let callback = RefCell::new(Some(callback));
    chooser.connect_response(move |dialog, response| {
        let paths = if response == gtk::ResponseType::Accept {
            let files = dialog.files();
            (0..files.n_items()).filter_map(|index| files.item(index).and_downcast::<gio::File>().and_then(|file| file.path())).collect()
        } else { Vec::new() };
        if let Some(callback) = callback.borrow_mut().take() { callback(paths); }
        dialog.destroy();
    });
    chooser.show();
}

fn confirm(parent: &ApplicationWindow, title: &str, message: &str, callback: impl FnOnce(bool) + 'static) {
    let dialog = gtk::MessageDialog::builder().transient_for(parent).modal(true).message_type(gtk::MessageType::Warning).buttons(gtk::ButtonsType::YesNo).text(title).secondary_text(message).build();
    let callback = RefCell::new(Some(callback));
    dialog.connect_response(move |dialog, response| { if let Some(callback) = callback.borrow_mut().take() { callback(response == gtk::ResponseType::Yes); } dialog.close(); });
    dialog.show();
}

fn text_input(parent: &ApplicationWindow, title: &str, prompt: &str, callback: impl FnOnce(Option<String>) + 'static) {
    let dialog = gtk::Dialog::builder().transient_for(parent).modal(true).title(title).build();
    dialog.add_button("Cancel", gtk::ResponseType::Cancel);
    dialog.add_button("Apply", gtk::ResponseType::Accept);
    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&Label::new(Some(prompt)));
    let entry = gtk::Entry::new();
    entry.set_activates_default(true);
    content.append(&entry);
    dialog.set_default_response(gtk::ResponseType::Accept);
    let entry_clone = entry.clone();
    let callback = RefCell::new(Some(callback));
    dialog.connect_response(move |dialog, response| {
        let value = if response == gtk::ResponseType::Accept { Some(entry_clone.text().to_string()) } else { None };
        if let Some(callback) = callback.borrow_mut().take() { callback(value); }
        dialog.close();
    });
    dialog.show();
    entry.grab_focus();
}

fn error_dialog(parent: &ApplicationWindow, message: &str) {
    let dialog = gtk::MessageDialog::builder().transient_for(parent).modal(true).message_type(gtk::MessageType::Error).buttons(gtk::ButtonsType::Close).text("Operation failed").secondary_text(message).build();
    dialog.connect_response(|dialog, _| dialog.close());
    dialog.show();
}

fn image_information_dialog(parent: &ApplicationWindow, image: &Path, report: &str) {
    let dialog = gtk::Dialog::builder().transient_for(parent).modal(true)
        .title("Image Information").default_width(760).default_height(620).build();
    dialog.add_button("Close", gtk::ResponseType::Close);
    dialog.connect_response(|dialog, _| dialog.close());
    let content = GtkBox::new(Orientation::Vertical, 16);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(20);
    content.set_margin_end(20);
    let heading = Label::new(Some(image.file_name().and_then(|name| name.to_str()).unwrap_or("ISO image")));
    heading.add_css_class("title-2");
    heading.set_xalign(0.0);
    content.append(&heading);
    let mut sections = backend::information_sections(report);
    let mut file = vec![("Location".into(), image.display().to_string())];
    if let Ok(metadata) = image.metadata() {
        file.push(("File size".into(), format!("{} ({} bytes)", format_size(metadata.len()), metadata.len())));
    }
    sections.insert(0, ("Image file".into(), file));
    for (title, fields) in sections {
        let section = GtkBox::new(Orientation::Vertical, 8);
        let title = Label::new(Some(&title));
        title.add_css_class("heading");
        title.set_xalign(0.0);
        section.append(&title);
        let grid = gtk::Grid::builder().column_spacing(20).row_spacing(8).build();
        for (index, (name, value)) in fields.iter().enumerate() {
            let key = Label::new(Some(name));
            key.set_xalign(0.0);
            key.set_valign(gtk::Align::Start);
            key.add_css_class("dim-label");
            let value = Label::new(Some(value));
            value.set_xalign(0.0);
            value.set_wrap(true);
            value.set_selectable(true);
            value.set_hexpand(true);
            grid.attach(&key, 0, index as i32, 1, 1);
            grid.attach(&value, 1, index as i32, 1, 1);
        }
        section.append(&grid);
        content.append(&section);
    }
    let details = gtk::Expander::new(Some("Full xorriso report"));
    let text = gtk::TextView::new();
    text.set_editable(false);
    text.set_cursor_visible(false);
    text.set_monospace(true);
    text.set_wrap_mode(gtk::WrapMode::WordChar);
    text.buffer().set_text(report);
    details.set_child(Some(&text));
    content.append(&details);
    let scroll = ScrolledWindow::builder().hexpand(true).vexpand(true).child(&content).build();
    dialog.content_area().append(&scroll);
    dialog.present();
}

fn action_button(label: &str, icon: &str, style: &str, tooltip: &str) -> Button {
    let button = Button::new();
    let content = GtkBox::new(Orientation::Horizontal, 7);
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(16);
    content.append(&image);
    content.append(&Label::new(Some(label)));
    button.set_child(Some(&content));
    button.set_tooltip_text(Some(tooltip));
    button.add_css_class("toolbar-action");
    button.add_css_class(style);
    button.add_css_class("flat");
    button
}

fn dropdown_menu(label: &str, actions: &[(&str, &Button)]) -> gtk::MenuButton {
    let button = gtk::MenuButton::new();
    button.set_label(label);
    button.set_tooltip_text(Some(&format!("{label} commands")));
    button.add_css_class("flat");
    button.set_popover(Some(&command_popover(actions)));
    button
}

fn command_popover(actions: &[(&str, &Button)]) -> gtk::PopoverMenu {
    let model = gio::Menu::new();
    let group = gio::SimpleActionGroup::new();
    for (index, (label, button)) in actions.iter().enumerate() {
        let name = format!("item{index}");
        let action = gio::SimpleAction::new(&name, None);
        action.set_enabled(button.is_sensitive());
        let target = (*button).clone();
        action.connect_activate(move |_, _| target.emit_clicked());
        let action_for_state = action.clone();
        button.connect_sensitive_notify(move |button| action_for_state.set_enabled(button.is_sensitive()));
        group.add_action(&action);
        model.append(Some(label), Some(&format!("commands.{name}")));
    }
    let popover = gtk::PopoverMenu::from_model(Some(&model));
    popover.insert_action_group("commands", Some(&group));
    popover.set_has_arrow(false);
    popover.set_halign(gtk::Align::Start);
    popover.add_css_class("isoweaver-menu");
    popover
}

fn install_theme() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        ".menu-panel { padding: 4px 0; border-bottom: 1px solid alpha(currentColor, 0.10); }\
         .menu-panel menubutton > button { padding: 6px 12px; }\
         .isoweaver-menu contents { padding: 6px; }\
         .isoweaver-menu scrollbar.vertical, .isoweaver-menu scrollbar.vertical slider { min-height: 0; }\
         .isoweaver-menu modelbutton { min-height: 32px; padding: 2px 12px; border-radius: 6px; }\
         .toolbar-row button { padding: 6px 10px; }\
         .toolbar-row separator { margin: 4px 6px; }\
         .toolbar-action image { -gtk-icon-size: 16px; }\
         .session-banner { padding: 8px 0; border-bottom: 1px solid alpha(currentColor, 0.10); }\
         .image-title { font-weight: 700; }\
         .status-badge { opacity: 0.70; }\
         .status-text { padding: 8px 0; }\
         .isoweaver-sidebar { background: alpha(currentColor, 0.035); border-right: 1px solid alpha(currentColor, 0.10); }\
         .directory-tree { background: transparent; }\
         .directory-tree row { border-radius: 6px; margin: 2px 6px; }\
         .section-heading, .column-heading { font-size: 0.90em; font-weight: 600; opacity: 0.70; }\
         .file-list-header { border-bottom: 1px solid alpha(currentColor, 0.10); }\
         .file-list row { border-radius: 4px; margin: 1px 4px; }\
         .operation-panel { padding-top: 8px; border-top: 1px solid alpha(currentColor, 0.10); }",
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
}

fn valid_leaf(value: &str) -> bool {
    !value.is_empty() && value != "." && value != ".." && !value.contains('/') && !value.contains('\0')
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 { return format!("{bytes} B"); }
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 { size /= 1024.0; unit += 1; }
    format!("{size:.1} {}", UNITS[unit])
}

fn clear_list(list: &ListBox) {
    while let Some(row) = list.row_at_index(0) { list.remove(&row); }
}

// Handle both click modes before GTK's default multiple-selection gesture.
fn select_clicked_row(list: &ListBox, row: &ListBoxRow, control: bool) {
    if control && row.is_selected() {
        list.unselect_row(row);
    } else {
        if !control {
            list.unselect_all();
        }
        list.select_row(Some(row));
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    #[test]
    fn traditional_file_selection() {
        gtk::init().expect("GTK display required for selection test");
        gio::resources_register_include!("icons.gresource").unwrap();
        let theme = gtk::IconTheme::for_display(&gtk::gdk::Display::default().unwrap());
        theme.add_resource_path("/org/isoweaver/IsoWeaver/icons");
        assert!(theme.has_icon("org.isoweaver.IsoWeaver"));
        let folder = std::env::temp_dir().join(format!("isoweaver-name-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let source = folder.join("example.iso");
        let first = dated_image_path(&source).unwrap();
        assert!(first.file_name().unwrap().to_string_lossy().starts_with("example_edited_"));
        assert_ne!(first, source);
        std::fs::write(&first, "existing").unwrap();
        assert_ne!(dated_image_path(&source).unwrap(), first);
        std::fs::remove_dir_all(folder).unwrap();
        install_theme();
        let command = Button::with_label("Test command");
        let activated = Rc::new(RefCell::new(0));
        let count = activated.clone();
        command.connect_clicked(move |_| *count.borrow_mut() += 1);
        let menu = command_popover(&[("_Test Command", &command)]);
        let host = gtk::Window::new();
        let anchor = dropdown_menu("Test", &[("_Test Command", &command)]);
        host.set_child(Some(&anchor));
        host.present();
        anchor.popup();
        let until = std::time::Instant::now() + std::time::Duration::from_millis(250);
        while std::time::Instant::now() < until { glib::MainContext::default().iteration(false); }
        let popover = anchor.popover().unwrap();
        let scroller = popover.first_child().unwrap().first_child().unwrap();
        let viewport = scroller.first_child().unwrap();
        let natural = viewport.measure(Orientation::Vertical, -1).1;
        assert!(viewport.height() <= natural + 2, "Menu has {}px of unused bottom space", viewport.height() - natural);
        host.destroy();

        menu.activate_action("commands.item0", None).unwrap();
        assert_eq!(*activated.borrow(), 1);
        command.set_sensitive(false);
        menu.activate_action("commands.item0", None).unwrap();
        assert_eq!(*activated.borrow(), 1);
        command.set_sensitive(true);
        menu.activate_action("commands.item0", None).unwrap();
        assert_eq!(*activated.borrow(), 2);
        let list = ListBox::new();
        list.set_selection_mode(SelectionMode::Multiple);
        let rows: Vec<_> = (0..3).map(|_| {
            let row = ListBoxRow::new();
            list.append(&row);
            row
        }).collect();
        let selected = || list.selected_rows().iter().map(|row| row.index()).collect::<Vec<_>>();
        select_clicked_row(&list, &rows[0], false);
        assert_eq!(selected(), vec![0]);
        select_clicked_row(&list, &rows[0], false);
        assert_eq!(selected(), vec![0]);
        select_clicked_row(&list, &rows[1], true);
        assert_eq!(selected(), vec![0, 1]);
        select_clicked_row(&list, &rows[0], true);
        assert_eq!(selected(), vec![1]);
        select_clicked_row(&list, &rows[2], true);
        assert_eq!(selected(), vec![1, 2]);
        select_clicked_row(&list, &rows[1], false);
        assert_eq!(selected(), vec![1]);
        select_clicked_row(&list, &rows[2], false);
        assert_eq!(selected(), vec![2]);
        let fixture = std::env::temp_dir().join(format!("isoweaver-add-test-{}", std::process::id()));
        std::fs::create_dir_all(fixture.join("folder/nested")).unwrap();
        std::fs::write(fixture.join("folder/nested/hello.txt"), "hello").unwrap();
        let preview = AddPreview {
            all: Rc::new(RefCell::new(Vec::new())), visible: Rc::new(RefCell::new(Vec::new())),
            dir: Rc::new(RefCell::new(String::new())), tree: ListBox::new(),
            tree_paths: Rc::new(RefCell::new(Vec::new())), list: ListBox::new(),
            breadcrumb: Label::new(None), search: gtk::SearchEntry::new(),
        };
        preview.search.set_text("hidden-by-filter");
        let pending = Rc::new(RefCell::new(Vec::new()));
        stage_adds(&[(fixture.join("folder"), "/folder".into())], &pending,
            &Label::new(None), &Label::new(None), &preview);
        assert_eq!(pending.borrow().len(), 1);
        assert_eq!(preview.visible.borrow()[0].path, "folder");
        assert!(preview.tree_paths.borrow().contains(&"folder/nested".into()));
        let added = preview.all.borrow().iter().find(|entry| entry.path.ends_with("hello.txt")).unwrap().clone();
        assert_eq!(added.size, 5);
        let output = fixture.join("output");
        std::fs::create_dir(&output).unwrap();
        extract_entries(Path::new("unused.iso"), &output, &[added]).unwrap();
        assert_eq!(std::fs::read_to_string(output.join("folder/nested/hello.txt")).unwrap(), "hello");
        std::fs::write(fixture.join("folder/nested/hello.txt"), "replacement").unwrap();
        stage_adds(&[(fixture.join("folder"), "/folder".into())], &pending,
            &Label::new(None), &Label::new(None), &preview);
        assert_eq!(pending.borrow().len(), 1);
        assert_eq!(preview.all.borrow().iter().filter(|entry| entry.path.ends_with("hello.txt")).count(), 1);
        assert_eq!(preview.all.borrow().iter().find(|entry| entry.path.ends_with("hello.txt")).unwrap().size, 11);
        std::fs::remove_dir_all(fixture).unwrap();
        let app = Application::builder().application_id("org.isoweaver.SelectionTest").build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let window = ApplicationWindow::builder().application(&app).build();
        let completed = Rc::new(RefCell::new(false));
        let result = completed.clone();
        run_operation(&window, "Test operation", || {
            std::thread::sleep(std::time::Duration::from_millis(160));
            Ok(42)
        }, move |value| { assert_eq!(value.unwrap(), 42); *result.borrow_mut() = true; });
        let context = glib::MainContext::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !*completed.borrow() && std::time::Instant::now() < deadline {
            context.iteration(false);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(*completed.borrow(), "Worker completion must reach the GTK loop");
        window.destroy();
        build_ui(&app);
        let browser = app.active_window().unwrap();
        fn find_status(widget: &gtk::Widget, footer: bool) -> Option<Label> {
            if let Ok(label) = widget.clone().downcast::<Label>() {
                if (!footer && widget.has_css_class("status-badge")) || (footer && label.text().starts_with("Ready · Open")) { return Some(label); }
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                if let Some(label) = find_status(&widget, footer) { return Some(label); }
                child = widget.next_sibling();
            }
            None
        }
        for footer in [false, true] {
        let badge = find_status(browser.upcast_ref(), footer).unwrap();
        for scale in [1.0, 1.25, 1.5] {
        let attributes = gtk::pango::AttrList::new();
        attributes.insert(gtk::pango::AttrFloat::new_scale(scale));
        badge.set_attributes(Some(&attributes));
        badge.set_text(if footer { "125 entries · Select files to extract or edit" } else { "ISO image · Clean" });
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(250);
        while std::time::Instant::now() < deadline { context.iteration(false); }
        let (ink, _) = badge.layout().pixel_extents();
        let (_, offset) = badge.layout_offsets();
        let offset = offset + i32::from(badge.style_context().padding().top());
        assert!(offset + ink.y() >= 4, "Image status text has insufficient top clearance: {}", offset + ink.y());
        assert!(offset + ink.y() + ink.height() <= badge.allocated_height() - 4, "Image status text clips at the bottom: ink_end={}, height={}, allocated={}", offset + ink.y() + ink.height(), badge.height(), badge.allocated_height());
        }
        }
        browser.destroy();
    }
}

fn dated_image_path(source: &Path) -> Result<PathBuf, String> {
    let stamp = glib::DateTime::now_local().and_then(|date| date.format("%Y-%m-%d_%H-%M-%S"))
        .map_err(|error| error.to_string())?;
    let stem = source.file_stem().unwrap_or_default().to_string_lossy();
    for counter in 0..10000 {
        let extra = if counter == 0 { String::new() } else { format!("_{counter}") };
        let target = source.with_file_name(format!("{stem}_edited_{stamp}{extra}.iso"));
        if !target.exists() { return Ok(target); }
    }
    Err("Could not find an unused output filename.".into())
}

/// Poll a worker from the GTK loop; unknown subprocess durations use honest pulsing progress.
fn run_operation<T: Send + 'static>(parent: &ApplicationWindow, title: &str,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
    done: impl FnOnce(Result<T, String>) + 'static) {
    let dialog = gtk::Window::builder().transient_for(parent).modal(true)
        .title(title).resizable(false).default_width(380).build();
    dialog.connect_close_request(|_| glib::Propagation::Stop);
    let content = GtkBox::new(Orientation::Vertical, 16);
    for set_margin in [GtkBox::set_margin_top, GtkBox::set_margin_bottom, GtkBox::set_margin_start, GtkBox::set_margin_end] {
        set_margin(&content, 24);
    }
    content.append(&Label::new(Some(title)));
    let progress = ProgressBar::new();
    progress.pulse();
    content.append(&progress);
    dialog.set_child(Some(&content));
    dialog.present();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || { let _ = sender.send(work()); });
    let mut done = Some(done);
    glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
        progress.pulse();
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => Err("The operation stopped unexpectedly.".into()),
        };
        dialog.destroy();
        if let Some(done) = done.take() { done(result); }
        glib::ControlFlow::Break
    });
}
