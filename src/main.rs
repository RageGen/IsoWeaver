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

    install_theme();
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
    let save = action_button("Save", "document-save-symbolic", "action-save", "Save changes to the current image");
    let save_as = action_button("Save As", "document-save-as-symbolic", "action-save", "Save changes to a new image file");
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

    let edit_menu = [("Rename Selected", &rename), ("New Folder in Image", &create_directory)];
    let view_menu = [("Search image contents", &search_action)];
    let tools_menu = [("Image Information", &image_info)];
    let help_menu = [("About IsoWeaver", &about_action)];
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
    for button in [&open, &save, &save_as] { toolbar.append(button); }
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
    sidebar.set_margin_start(12);
    sidebar.set_margin_end(8);
    sidebar.set_margin_top(12);
    sidebar.add_css_class("isoweaver-sidebar");
    let tree_heading = Label::new(Some("IMAGE TREE"));
    tree_heading.set_xalign(0.0);
    tree_heading.add_css_class("section-heading");
    sidebar.append(&tree_heading);
    sidebar.append(&tree_scroll);

    let file_list = ListBox::new();
    file_list.set_selection_mode(SelectionMode::Multiple);
    file_list.set_tooltip_text(Some("Click to select; Ctrl-click to add or remove files from the selection"));
    file_list.add_css_class("file-list");
    let ctrl_click = gtk::GestureClick::new();
    ctrl_click.set_button(1);
    ctrl_click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let list_for_ctrl_click = file_list.clone();
    ctrl_click.connect_pressed(move |gesture, _, _, y| {
        if gesture.current_event_state().contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            if let Some(row) = list_for_ctrl_click.row_at_y(y.floor() as i32) {
                if row.is_selected() {
                    list_for_ctrl_click.unselect_row(&row);
                } else {
                    list_for_ctrl_click.select_row(Some(&row));
                }
                let _ = gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        }
    });
    file_list.add_controller(ctrl_click);
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
    let selection_status = status.clone();
    file_list.connect_selected_rows_changed(move |list| {
        let count = list.selected_rows().len();
        if count > 0 {
            selection_status.set_text(&format!("{count} selected · Ctrl-click to add or remove"));
        }
    });
    status.set_hexpand(true);
    let progress = ProgressBar::new();
    progress.set_show_text(true);
    progress.set_text(Some("Operations appear here"));
    progress.set_fraction(0.0);
    progress.set_size_request(220, -1);
    let cancel = Button::with_label("Cancel");
    cancel.set_sensitive(false);
    let operation_panel = GtkBox::new(Orientation::Horizontal, 12);
    operation_panel.set_margin_start(16);
    operation_panel.set_margin_end(16);
    operation_panel.set_margin_top(10);
    operation_panel.set_margin_bottom(12);
    operation_panel.add_css_class("operation-panel");
    operation_panel.append(&status);
    operation_panel.append(&progress);
    operation_panel.append(&cancel);
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
        let about = gtk::AboutDialog::builder().transient_for(&about_parent).modal(true).program_name("IsoWeaver").version(env!("CARGO_PKG_VERSION")).comments("A native Linux application for browsing, extracting, and editing ISO images.").build();
        if let Ok(logo_pixbuf) = gdk_pixbuf::Pixbuf::from_read(
            std::io::Cursor::new(include_bytes!("../data/icons/256x256/apps/org.isoweaver.IsoWeaver.png")),
        ) {
            let logo = gtk::gdk::Texture::for_pixbuf(&logo_pixbuf);
            about.set_logo(Some(&logo));
        }
        about.set_copyright(Some("Copyright (C) 2026 IsoWeaver contributors"));
        about.set_license_type(gtk::License::Custom);
        about.set_license(Some("IsoWeaver is licensed under GPL-3.0-or-later. The complete license text is included in the LICENSE file."));
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
    let status_add = status.clone();
    let state_add = image_state_label.clone();
    add_files.connect_clicked(move |_| {
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
                    if yes { stage_adds(&candidates, &pending, &status, &image_state); }
                });
            } else {
                stage_adds(&candidates, &pending, &status, &image_state);
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
                    if yes { stage_adds(&candidates, &pending, &status, &image_state); }
                });
            } else {
                stage_adds(&candidates, &pending, &status, &image_state);
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
    let info_label = current_image_label.clone();
    image_info.connect_clicked(move |_| {
        let Some(image) = current.borrow().clone() else { return };
        match inspect_boot(&image) {
            Ok(report) => info_dialog(&parent, &format!("Image Information · {}", info_label.text()), &report),
            Err(error) => error_dialog(&parent, &error),
        }
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
                if let Err(error) = extract_entries(&image, &destination, &expanded) { error_dialog(&parent_for_callback, &error); }
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
                if let Err(error) = extract_entries(&image, &destination, &entries) { error_dialog(&parent_for_callback, &error); }
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
        let parent_for_callback = parent.clone();
        confirm(&parent, "Save image changes?", "The ISO will be rebuilt and replace the current image only after validation.", move |yes| {
            if yes { finish_save(&parent_for_callback, &image, &image, &staged, true, &state); }
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
    let window_for_save_as = window_state.clone();
    save_as.connect_clicked(move |_| {
        let Some(image) = current.borrow().clone() else { return };
        let staged = pending.borrow().clone();
        if staged.is_empty() { return; }
        let chooser = FileChooserNative::new(Some("Save modified image as"), Some(&window_for_save_as), FileChooserAction::Save, Some("Save"), Some("Cancel"));
        chooser.set_current_name(image.file_name().and_then(|name| name.to_str()).unwrap_or("image.iso"));
        let window_for_response = window_for_save_as.clone();
        let state = SaveUi { tree: tree.clone(), title: title.clone(), image_state: image_state.clone(), status: status_save.clone(), list: list.clone(), all: all.clone(), visible: visible.clone(), current: current.clone(), dir: dir.clone(), tree_paths: tree_paths.clone(), breadcrumb: breadcrumb.clone(), search: search_save.clone(), pending: pending.clone() };
        chooser.connect_response(move |dialog, response| {
            if response == gtk::ResponseType::Accept {
                if let Some(target) = dialog.file().and_then(|file| file.path()) {
                    let image = image.clone();
                    let staged = staged.clone();
                    let state = state.clone();
                    let parent = window_for_response.clone();
                    if target.exists() {
                        confirm(&window_for_response, "Replace existing image?", "The file will be replaced only after the rebuilt image is validated.", move |yes| {
                            if yes { finish_save(&parent, &image, &target, &staged, true, &state); }
                        });
                    } else {
                        finish_save(&window_for_response, &image, &target, &staged, false, &state);
                    }
                }
            }
            dialog.destroy();
        });
        chooser.show();
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
    match list_image(path) {
        Ok(mut entries) => {
            entries.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
            *current.borrow_mut() = Some(path.to_path_buf());
            *all.borrow_mut() = entries;
            *dir.borrow_mut() = String::new();
            title.set_text(path.file_name().and_then(|name| name.to_str()).unwrap_or("Image"));
            image_state.set_text("ISO image · Clean");
            render_tree(tree, &all.borrow(), tree_paths);
            render_files(file_list, &all.borrow(), visible, &dir.borrow(), &search.text(), breadcrumb);
            status.set_text(&format!("{} entries · Select files to extract or edit", all.borrow().len()));
        }
        Err(error) => {
            *current.borrow_mut() = None;
            all.borrow_mut().clear();
            visible.borrow_mut().clear();
            clear_list(file_list);
            clear_list(tree);
            tree_paths.borrow_mut().clear();
            title.set_text("No image open");
            image_state.set_text("Ready to open an image");
            breadcrumb.set_text("Image: /");
            status.set_text(&format!("Could not open image: {error}"));
        }
    }
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
        let kind = Label::new(Some(if entry.directory { "Directory" } else { file_type(&entry.path) }));
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

fn stage_adds(candidates: &[(PathBuf, String)], pending: &Rc<RefCell<Vec<Change>>>, status: &Label, image_state: &Label) {
    let mut changes = pending.borrow_mut();
    for (source, image_path) in candidates {
        if !changes.iter().any(|change| matches!(change, Change::Add { image_path: existing, .. } if existing == image_path)) {
            changes.push(Change::Add { source: source.clone(), image_path: image_path.clone() });
        }
    }
    status.set_text(&format!("Modified · {} pending changes", changes.len()));
    image_state.set_text("Modified (*) · Unsaved changes");
}

fn finish_save(parent: &ApplicationWindow, input: &Path, target: &Path, changes: &[Change], replace: bool, ui: &SaveUi) {
    ui.status.set_text("Rebuilding and validating image…");
    match save_image(input, target, changes, replace) {
        Ok(()) => {
            ui.pending.borrow_mut().clear();
            load_image(target, &ui.list, &ui.tree, &ui.title, &ui.image_state, &ui.status, &ui.current, &ui.all, &ui.visible, &ui.dir, &ui.tree_paths, &ui.breadcrumb, &ui.search);
            ui.image_state.set_text("ISO image · Clean");
            ui.status.set_text("Saved · rebuilt image reopened successfully");
            let _ = parent;
        }
        Err(error) => {
            ui.image_state.set_text("Modified · Save failed");
            ui.status.set_text("Save failed · original image preserved");
            error_dialog(parent, &error);
        }
    }
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

fn info_dialog(parent: &ApplicationWindow, title: &str, message: &str) {
    let dialog = gtk::MessageDialog::builder().transient_for(parent).modal(true).message_type(gtk::MessageType::Info).buttons(gtk::ButtonsType::Close).text(title).secondary_text(message).build();
    dialog.connect_response(|dialog, _| dialog.close());
    dialog.show();
}

fn action_button(label: &str, icon: &str, style: &str, tooltip: &str) -> Button {
    let button = Button::new();
    let content = GtkBox::new(Orientation::Horizontal, 7);
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(18);
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

fn command_popover(actions: &[(&str, &Button)]) -> gtk::Popover {
    let popover = gtk::Popover::new();
    let items = GtkBox::new(Orientation::Vertical, 2);
    items.set_margin_top(6);
    items.set_margin_bottom(6);
    items.set_margin_start(6);
    items.set_margin_end(6);
    for (label, action) in actions {
        let item = Button::with_label(label);
        item.set_halign(gtk::Align::Fill);
        item.add_css_class("flat");
        let target = (*action).clone();
        let popover_for_click = popover.clone();
        item.connect_clicked(move |_| { popover_for_click.popdown(); target.emit_clicked(); });
        items.append(&item);
    }
    popover.set_child(Some(&items));
    popover
}

fn install_theme() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        ".menu-panel { padding: 2px 4px; border-bottom: 1px solid alpha(currentColor, 0.10); }\
         .menu-panel menubutton { padding: 3px 8px; }\
         .toolbar-row button { padding: 7px 10px; }\
         .toolbar-action image { -gtk-icon-size: 18px; }\
         .action-open image { color: #3584e4; }\
         .action-save image { color: #26a269; }\
         .action-add image { color: #9141ac; }\
         .action-extract image { color: #e5a50a; }\
         .action-delete image { color: #c01c28; }\
         .action-edit image { color: #613583; }\
         .session-banner { margin-top: 2px; padding: 12px 16px; border-radius: 12px; background: alpha(@accent_bg_color, 0.12); }\
         .image-title { font-weight: 700; font-size: 1.05em; }\
         .status-badge { padding: 5px 10px; border-radius: 999px; background: alpha(@accent_bg_color, 0.18); font-weight: 600; }\
         .isoweaver-sidebar { border-right: 1px solid alpha(currentColor, 0.10); }\
         .section-heading, .column-heading { font-size: 0.82em; font-weight: 700; letter-spacing: 0.04em; opacity: 0.75; }\
         .file-list-header { border-bottom: 1px solid alpha(currentColor, 0.12); }\
         .file-list row { border-bottom: 1px solid alpha(currentColor, 0.06); }\
         .file-list row:selected { border-radius: 8px; }\
         .operation-panel { padding-top: 8px; border-top: 1px solid alpha(currentColor, 0.12); }",
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
