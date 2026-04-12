use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
    Frame, Terminal,
};
use std::{
    fs,
    io::{self, stdout},
    path::PathBuf,
};

#[derive(PartialEq)]
enum AppMode {
    Normal,
    Insert,
    Command,
    ShowHelp,
    PromptNewFile,
    PromptRenameFile,
    SearchFile,
    Visual,
}

struct App {
    mode: AppMode,
    notes_dir: PathBuf,
    files: Vec<PathBuf>,
    file_list_state: ListState,
    active_file: Option<PathBuf>,
    content: String,
    cursor_index: usize,
    visual_anchor: Option<usize>,
    clipboard: String,
    command_buffer: String,
    prompt_buffer: String,
    search_query: String,
    status_message: String,
    editor_scroll_y: u16,
    editor_scroll_x: u16,
}

impl App {
    fn new() -> Self {
        let mut path = home::home_dir().expect("Could not find home directory");
        path.push(".local");
        path.push("share");
        path.push("mynotes");

        if !path.exists() {
            let _ = fs::create_dir_all(&path);
        }

        let mut app = Self {
            mode: AppMode::Normal,
            notes_dir: path,
            files: Vec::new(),
            file_list_state: ListState::default(),
            active_file: None,
            content: String::new(),
            cursor_index: 0,
            visual_anchor: None,
            clipboard: String::new(),
            command_buffer: String::new(),
            prompt_buffer: String::new(),
            search_query: String::new(),
            status_message: String::new(),
            editor_scroll_y: 0,
            editor_scroll_x: 0,
        };

        app.refresh_files();
        if !app.files.is_empty() {
            app.file_list_state.select(Some(0));
            app.load_file();
        }

        app
    }

    fn refresh_files(&mut self) {
        self.files.clear();
        if let Ok(entries) = fs::read_dir(&self.notes_dir) {
            let q = self.search_query.to_lowercase();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "md") {
                    let fname = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                    
                    let matches = if self.search_query.is_empty() {
                        true
                    } else if fname.contains(&q) {
                        true
                    } else {
                        // Check content if filename doesn't match
                        if let Ok(content) = fs::read_to_string(&path) {
                            content.to_lowercase().contains(&q)
                        } else {
                            false
                        }
                    };

                    if matches {
                        self.files.push(path);
                    }
                }
            }
        }
        self.files.sort();
        if let Some(selected) = self.file_list_state.selected() {
            if self.files.is_empty() {
                self.file_list_state.select(None);
            } else if selected >= self.files.len() {
                self.file_list_state.select(Some(self.files.len() - 1));
            }
        }
    }

    fn load_file(&mut self) {
        if let Some(selected) = self.file_list_state.selected() {
            let path_opt = self.files.get(selected).cloned();
            if let Some(path) = path_opt {
                let needs_load = match &self.active_file {
                    Some(active) => active != &path,
                    None => true,
                };

                if needs_load {
                    if self.active_file.is_some() {
                        self.save_file();
                    }
                    self.active_file = Some(path.clone());
                    self.content = fs::read_to_string(&path).unwrap_or_default();
                    self.cursor_index = 0; // Better reset to 0 instead of end for navigating big files
                    self.editor_scroll_y = 0;
                    self.editor_scroll_x = 0;
                    self.visual_anchor = None;
                    self.status_message = format!("Loaded: {}", path.file_name().unwrap_or_default().to_string_lossy());
                }
            }
        }
    }

    fn save_file(&mut self) {
        if let Some(path) = &self.active_file {
            if let Err(e) = fs::write(path, &self.content) {
                self.status_message = format!("Error saving: {}", e);
            } else {
                self.status_message = String::from("Saved.");
            }
        }
    }

    fn delete_file(&mut self) {
        if let Some(selected) = self.file_list_state.selected() {
            if let Some(path) = self.files.get(selected) {
                if let Err(e) = fs::remove_file(path) {
                    self.status_message = format!("Error deleting: {}", e);
                } else {
                    self.status_message = format!("Deleted: {}", path.file_name().unwrap_or_default().to_string_lossy());
                    self.refresh_files();
                    self.active_file = None;
                    self.content.clear();
                    self.cursor_index = 0;
                    self.editor_scroll_y = 0;
                    
                    if !self.files.is_empty() {
                        let new_index = selected.saturating_sub(1);
                        self.file_list_state.select(Some(new_index));
                        self.load_file();
                    } else {
                         self.file_list_state.select(None);
                    }
                }
            }
        }
    }

    fn create_new_file(&mut self) {
        if self.prompt_buffer.is_empty() {
            self.mode = AppMode::Normal;
            self.status_message = "Cancelled.".into();
            return;
        }

        let mut filename = self.prompt_buffer.trim().to_string();
        if !filename.ends_with(".md") {
            filename.push_str(".md");
        }

        let mut path = self.notes_dir.clone();
        path.push(&filename);

        if path.exists() {
            self.status_message = format!("Error: {} already exists.", filename);
            self.prompt_buffer.clear();
            self.mode = AppMode::Normal;
            return;
        }

        if let Err(e) = fs::write(&path, "") {
            self.status_message = format!("Error: {}", e);
        } else {
            if self.active_file.is_some() {
                self.save_file();
            }
            // Clear search filter so we can see the new file
            self.search_query.clear();
            self.refresh_files();
            
            let new_index = self.files.iter().position(|p| p == &path).unwrap_or(0);
            self.file_list_state.select(Some(new_index));
            self.active_file = Some(path.clone());
            self.content.clear();
            self.cursor_index = 0;
            self.editor_scroll_y = 0;
            self.visual_anchor = None;

            self.status_message = format!("Created {}", filename);
            self.mode = AppMode::Insert;
        }
        self.prompt_buffer.clear();
    }

    fn rename_file(&mut self) {
        if self.prompt_buffer.is_empty() {
            self.mode = AppMode::Normal;
            self.status_message = "Cancelled rename.".into();
            return;
        }
        
        let mut filename = self.prompt_buffer.trim().to_string();
        if !filename.ends_with(".md") {
            filename.push_str(".md");
        }

        if let Some(active) = &self.active_file {
            let mut new_path = self.notes_dir.clone();
            new_path.push(&filename);
            
            if new_path.exists() {
                self.status_message = format!("Error: {} already exists.", filename);
            } else {
                if let Err(e) = fs::rename(active, &new_path) {
                    self.status_message = format!("Error renaming: {}", e);
                } else {
                    self.active_file = Some(new_path.clone());
                    self.refresh_files();
                    
                    let new_index = self.files.iter().position(|p| p == &new_path).unwrap_or(0);
                    self.file_list_state.select(Some(new_index));
                    self.status_message = format!("Renamed to {}", filename);
                }
            }
        }
        self.prompt_buffer.clear();
        self.mode = AppMode::Normal;
    }

    fn next_file(&mut self) {
        if self.files.is_empty() { return; }
        let i = match self.file_list_state.selected() {
            Some(i) => if i >= self.files.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.file_list_state.select(Some(i));
        self.load_file();
    }

    fn previous_file(&mut self) {
        if self.files.is_empty() { return; }
        let i = match self.file_list_state.selected() {
            Some(i) => if i == 0 { self.files.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.file_list_state.select(Some(i));
        self.load_file();
    }

    // --- Cursor Tracking & Scroll Updates ---
    
    // Updates internal visual_scroll metrics based on requested view window size 
    fn update_scroll_to_cursor(&mut self, view_height: u16, _view_width: u16) {
        let (_, row) = self.get_visual_cursor();
        if row < self.editor_scroll_y {
            self.editor_scroll_y = row;
        } else if row >= self.editor_scroll_y + view_height {
            self.editor_scroll_y = row.saturating_sub(view_height).saturating_add(1);
        }
    }

    // --- Editor Helpers ---
    fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor_index, c);
        self.cursor_index += c.len_utf8();
    }

    fn insert_string(&mut self, s: &str) {
        self.content.insert_str(self.cursor_index, s);
        self.cursor_index += s.len();
    }

    fn delete_char(&mut self) {
        if self.cursor_index > 0 {
            let mut index = self.cursor_index - 1;
            while !self.content.is_char_boundary(index) { index -= 1; }
            self.content.remove(index);
            self.cursor_index = index;
        }
    }

    fn move_cursor_left(&mut self) {
        if self.cursor_index > 0 {
            let mut index = self.cursor_index - 1;
            while !self.content.is_char_boundary(index) { index -= 1; }
            self.cursor_index = index;
        }
    }

    fn move_cursor_right(&mut self) {
        if self.cursor_index < self.content.len() {
            let mut index = self.cursor_index + 1;
            while !self.content.is_char_boundary(index) { index += 1; }
            self.cursor_index = index;
        }
    }
    
    fn move_cursor_up(&mut self) {
        let (col, row) = self.get_visual_cursor();
        if row > 0 {
            let lines: Vec<&str> = self.content.split('\n').collect();
            let prev_line = lines[row as usize - 1];
            let new_col = col.min(prev_line.chars().count() as u16);
            
            let mut new_idx = 0;
            for i in 0..(row as usize - 1) {
                new_idx += lines[i].len() + 1;
            }
            new_idx += prev_line.chars().take(new_col as usize).map(|c| c.len_utf8()).sum::<usize>();
            self.cursor_index = new_idx;
        }
    }

    fn move_cursor_down(&mut self) {
        let (col, row) = self.get_visual_cursor();
        let lines: Vec<&str> = self.content.split('\n').collect();
        if (row as usize) < lines.len().saturating_sub(1) {
            let next_line = lines[row as usize + 1];
            let new_col = col.min(next_line.chars().count() as u16);
            
            let mut new_idx = 0;
            for i in 0..=(row as usize) {
                new_idx += lines[i].len() + 1;
            }
            new_idx += next_line.chars().take(new_col as usize).map(|c| c.len_utf8()).sum::<usize>();
            self.cursor_index = new_idx;
        }
    }

    fn get_visual_cursor(&self) -> (u16, u16) {
        let text_before_cursor = &self.content[..self.cursor_index];
        let lines: Vec<&str> = text_before_cursor.split('\n').collect();
        let row = (lines.len() as u16).saturating_sub(1);
        let col = lines.last().map(|l| l.chars().count()).unwrap_or(0) as u16;
        (col, row)
    }

    fn enter_visual_mode(&mut self) {
        if self.active_file.is_some() {
            self.mode = AppMode::Visual;
            self.visual_anchor = Some(self.cursor_index);
            self.status_message = "Visual Selection".into();
        }
    }

    fn perform_yank(&mut self) {
        if let Some(anchor) = self.visual_anchor {
            let start = anchor.min(self.cursor_index);
            let end = anchor.max(self.cursor_index);
            self.clipboard = self.content[start..end].to_string();
            self.mode = AppMode::Normal;
            self.visual_anchor = None;
            self.status_message = format!("Yanked {} bytes.", self.clipboard.len());
        }
    }

    fn perform_cut(&mut self) {
        if let Some(anchor) = self.visual_anchor {
            let start = anchor.min(self.cursor_index);
            let end = anchor.max(self.cursor_index);
            self.clipboard = self.content[start..end].to_string();
            // Drain range
            self.content.drain(start..end);
            self.cursor_index = start;
            self.mode = AppMode::Normal;
            self.visual_anchor = None;
            self.status_message = format!("Cut {} bytes.", self.clipboard.len());
            self.save_file();
        }
    }
}

// UI Helper
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new();

    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(std::time::Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.mode {
                        AppMode::Normal => match (key.code, key.modifiers) {
                            (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                                app.save_file();
                                return Ok(());
                            }
                            (KeyCode::Char('s'), KeyModifiers::CONTROL) => app.save_file(),
                            (KeyCode::Char('j'), _) | (KeyCode::Down, _) => app.next_file(),
                            (KeyCode::Char('k'), _) | (KeyCode::Up, _) => app.previous_file(),
                            (KeyCode::Enter, _) => app.load_file(),
                            (KeyCode::Char('r'), _) => {
                                if let Some(ref path) = app.active_file {
                                    app.mode = AppMode::PromptRenameFile;
                                    app.prompt_buffer = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                                }
                            }
                            // Search / Filter
                            (KeyCode::Char('/'), _) => {
                                app.mode = AppMode::SearchFile;
                                app.prompt_buffer = app.search_query.clone();
                            }
                            (KeyCode::Char('n'), _) => {
                                app.mode = AppMode::PromptNewFile;
                                app.prompt_buffer.clear();
                            }
                            (KeyCode::Char('v'), _) => app.enter_visual_mode(),
                            (KeyCode::Char('p'), _) => app.insert_string(&app.clipboard.clone()),
                            (KeyCode::Char('?'), _) | (KeyCode::F(1), _) => app.mode = AppMode::ShowHelp,
                            (KeyCode::Char('i'), _) => {
                                if app.active_file.is_some() {
                                    app.mode = AppMode::Insert;
                                    app.status_message = "Entering Insert Mode".into();
                                }
                            }
                            (KeyCode::Char('d'), _) => app.delete_file(),
                            (KeyCode::Char(':'), _) => {
                                app.mode = AppMode::Command;
                                app.command_buffer = ":".to_string();
                            }
                            _ => {}
                        },
                        AppMode::Insert => match (key.code, key.modifiers) {
                            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                                app.save_file();
                                return Ok(());
                            }
                            (KeyCode::Char('s'), KeyModifiers::CONTROL) => app.save_file(),
                            (KeyCode::Esc, _) => {
                                app.mode = AppMode::Normal;
                                app.status_message = "Normal Mode".into();
                            }
                            (KeyCode::Char(c), _) => app.insert_char(c),
                            (KeyCode::Enter, _) => app.insert_char('\n'),
                            (KeyCode::Backspace, _) => app.delete_char(),
                            (KeyCode::Left, _) => app.move_cursor_left(),
                            (KeyCode::Right, _) => app.move_cursor_right(),
                            (KeyCode::Up, _) => app.move_cursor_up(),
                            (KeyCode::Down, _) => app.move_cursor_down(),
                            _ => {}
                        },
                        AppMode::Visual => match (key.code, key.modifiers) {
                            (KeyCode::Esc, _) => {
                                app.mode = AppMode::Normal;
                                app.visual_anchor = None;
                                app.status_message = "Visual Selection Cancelled".into();
                            }
                            (KeyCode::Char('y'), _) => app.perform_yank(),
                            (KeyCode::Char('d'), _) => app.perform_cut(),
                            (KeyCode::Left, _) | (KeyCode::Char('h'), _) => app.move_cursor_left(),
                            (KeyCode::Right, _) | (KeyCode::Char('l'), _) => app.move_cursor_right(),
                            (KeyCode::Up, _) | (KeyCode::Char('k'), _) => app.move_cursor_up(),
                            (KeyCode::Down, _) | (KeyCode::Char('j'), _) => app.move_cursor_down(),
                            _ => {}
                        },
                        AppMode::Command => match (key.code, key.modifiers) {
                            (KeyCode::Esc, _) => {
                                app.mode = AppMode::Normal;
                                app.command_buffer.clear();
                            }
                            (KeyCode::Enter, _) => {
                                if app.command_buffer == ":w" {
                                    app.save_file();
                                } else if app.command_buffer == ":q" {
                                    app.save_file();
                                    return Ok(());
                                } else if app.command_buffer == ":wq" {
                                    app.save_file();
                                    return Ok(());
                                }
                                app.mode = AppMode::Normal;
                                app.command_buffer.clear();
                            }
                            (KeyCode::Char(c), _) => app.command_buffer.push(c),
                            (KeyCode::Backspace, _) => {
                                app.command_buffer.pop();
                                if app.command_buffer.is_empty() {
                                    app.mode = AppMode::Normal;
                                }
                            }
                            _ => {}
                        },
                        AppMode::ShowHelp => match key.code {
                            _ => app.mode = AppMode::Normal,
                        },
                        AppMode::PromptNewFile => match key.code {
                            KeyCode::Esc => {
                                app.mode = AppMode::Normal;
                                app.prompt_buffer.clear();
                            }
                            KeyCode::Enter => app.create_new_file(),
                            KeyCode::Char(c) => app.prompt_buffer.push(c),
                            KeyCode::Backspace => { app.prompt_buffer.pop(); },
                            _ => {}
                        },
                        AppMode::PromptRenameFile => match key.code {
                            KeyCode::Esc => {
                                app.mode = AppMode::Normal;
                                app.prompt_buffer.clear();
                            }
                            KeyCode::Enter => app.rename_file(),
                            KeyCode::Char(c) => app.prompt_buffer.push(c),
                            KeyCode::Backspace => { app.prompt_buffer.pop(); },
                            _ => {}
                        },
                        AppMode::SearchFile => match key.code {
                            KeyCode::Esc => {
                                app.mode = AppMode::Normal;
                                app.prompt_buffer.clear();
                                app.search_query.clear();
                                app.refresh_files();
                            }
                            KeyCode::Enter => {
                                app.mode = AppMode::Normal;
                                app.search_query = app.prompt_buffer.clone();
                                app.prompt_buffer.clear();
                                app.refresh_files();
                            }
                            KeyCode::Char(c) => app.prompt_buffer.push(c),
                            KeyCode::Backspace => { app.prompt_buffer.pop(); },
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

// Generate styled text with Markdown/Visual highlights
fn generate_highlighted_lines<'a>(app: &'a App) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    let mut byte_offset = 0;
    
    // Bounds check visual selection
    let (sel_min, sel_max) = match app.visual_anchor {
        Some(anchor) => (anchor.min(app.cursor_index), anchor.max(app.cursor_index)),
        None => (0, 0),
    };
    let is_visual = app.visual_anchor.is_some();

    for line_str in app.content.split('\n') {
        let line_len = line_str.len();
        let line_start = byte_offset;
        let line_end = byte_offset + line_len; // not including newline
        
        let mut base_style = Style::default().fg(Color::White);
        if line_str.starts_with("# ") || line_str.starts_with("## ") {
            base_style = base_style.fg(Color::LightBlue).add_modifier(Modifier::BOLD);
        } else if line_str.starts_with("> ") {
            base_style = base_style.fg(Color::Cyan).add_modifier(Modifier::ITALIC);
        } else if line_str.starts_with("- ") || line_str.starts_with("* ") {
            base_style = base_style.fg(Color::Yellow);
        }

        if !is_visual || sel_max <= line_start || sel_min >= line_end {
            // Entirely outside selection or no selection active
            lines.push(Line::from(Span::styled(line_str.to_string(), base_style)));
        } else {
            // Overlaps with selection
            let start_idx = sel_min.saturating_sub(line_start).min(line_len);
            let end_idx = sel_max.saturating_sub(line_start).min(line_len);

            let mut spans = Vec::new();
            if start_idx > 0 {
                spans.push(Span::styled(line_str[..start_idx].to_string(), base_style));
            }
            if start_idx < end_idx {
                spans.push(Span::styled(
                    line_str[start_idx..end_idx].to_string(), 
                    base_style.bg(Color::DarkGray)
                ));
            }
            if end_idx < line_len {
                spans.push(Span::styled(line_str[end_idx..].to_string(), base_style));
            }
            lines.push(Line::from(spans));
        }

        byte_offset += line_len + 1; // +1 to account for the \n dropped by split
    }
    
    lines
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    let top_bar = Paragraph::new(" NOTES ")
        .style(Style::default().bg(Color::Blue).fg(Color::Black).add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(top_bar, chunks[0]);

    let main_panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(chunks[1]);

    // --- Left Pane: File Explorer ---
    let items: Vec<ListItem> = app.files.iter()
        .map(|f| ListItem::new(f.file_name().unwrap_or_default().to_string_lossy().to_string()))
        .collect();

    let title = if app.mode == AppMode::Normal { " [Files] " } else { " Files " };
    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL).border_type(BorderType::Thick))
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, main_panes[0], &mut app.file_list_state);

    // --- Right Pane: Editor & Scrollbar ---
    let e_title = match app.mode {
        AppMode::Insert => " [Editor (INSERT)] ",
        AppMode::Visual => " [Editor (VISUAL)] ",
        _ => " Editor ",
    };
    
    let editor_block = Block::default()
        .title(e_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Thick);

    let inner_editor_area = editor_block.inner(main_panes[1]);
    
    // Update scroll window variables based on cursor to keep it in view
    if app.mode == AppMode::Insert || app.mode == AppMode::Visual {
        app.update_scroll_to_cursor(inner_editor_area.height, inner_editor_area.width);
    }
    
    let doc_lines = if app.active_file.is_some() {
        generate_highlighted_lines(app)
    } else {
        vec![Line::from("No note selected. Press 'n' to create one.")]
    };
    
    let total_lines = doc_lines.len();

    let paragraph = Paragraph::new(doc_lines)
        .block(editor_block)
        .style(Style::default().fg(Color::White))
        .scroll((app.editor_scroll_y, app.editor_scroll_x)); // Only inject vertical offset

    f.render_widget(paragraph, main_panes[1]);

    // Render Scrollbar
    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("▲"))
        .end_symbol(Some("▼"));
        
    let mut scrollbar_state = ScrollbarState::default()
        .content_length(total_lines)
        .position(app.editor_scroll_y as usize);
        
    f.render_stateful_widget(scrollbar, main_panes[1], &mut scrollbar_state);

    // Handle standard hardware cursor inside the terminal (Only in InsertMode usually)
    if (app.mode == AppMode::Insert || app.mode == AppMode::Visual) && app.active_file.is_some() {
        let (col, row) = app.get_visual_cursor();
        // Check if cursor is actually within the scrolling view bounds
        if row >= app.editor_scroll_y && row < app.editor_scroll_y + inner_editor_area.height {
            f.set_cursor_position((
                inner_editor_area.x + col - app.editor_scroll_x,
                inner_editor_area.y + row - app.editor_scroll_y,
            ));
        }
    }

    // --- Bottom Bar: Status Bar ---
    let mode_str = match app.mode {
        AppMode::Normal => " NORMAL ",
        AppMode::Insert => " INSERT ",
        AppMode::Command => " COMMAND ",
        AppMode::ShowHelp => " HELP ",
        AppMode::PromptNewFile => " PROMPT ",
        AppMode::PromptRenameFile => " RENAME ",
        AppMode::SearchFile => " SEARCH ",
        AppMode::Visual => " VISUAL ",
    };

    let file_info = match &app.active_file {
        Some(p) => p.file_name().unwrap_or_default().to_string_lossy(),
        None => "[No File]".into(),
    };

    let (filter_mark, color) = match app.mode {
        AppMode::Normal => (if app.search_query.is_empty() { "" } else { "[Filtered] " }, Color::Blue),
        AppMode::Insert => ("", Color::Green),
        AppMode::Command => ("", Color::Yellow),
        AppMode::Visual => ("", Color::Rgb(255, 165, 0)), // Orange
        AppMode::ShowHelp => ("", Color::Cyan),
        AppMode::SearchFile => ("", Color::LightYellow),
        _ => ("", Color::Magenta),
    };

    let status_text = match app.mode {
        AppMode::Command => format!("{}  {}", mode_str, app.command_buffer),
        _ => format!("{}{} | File: {} | (? for Help) | {}", mode_str, filter_mark, file_info, app.status_message),
    };

    let status_bar = Paragraph::new(status_text)
        .style(Style::default().bg(color).fg(Color::Black).add_modifier(Modifier::BOLD));

    f.render_widget(status_bar, chunks[2]);

    // --- Overlays ---
    if app.mode == AppMode::ShowHelp {
        let area = centered_rect(50, 60, f.area());
        let help_text = "\
        [ Navigation ]\n\
        j/Down, k/Up : Move file list\n\
        Enter        : Open highlighted file\n\
        /            : Search/Filter file list\n\
        \n\
        [ File Operations ]\n\
        n            : Create new file\n\
        r            : Rename active file\n\
        d            : Delete active file\n\
        :w / C-s     : Save file\n\
        \n\
        [ Editor ]\n\
        i            : Enter Insert Mode\n\
        v            : Enter Visual Selection\n\
        y (in vis)   : Yank (Copy) text\n\
        d (in vis)   : Cut text\n\
        p            : Paste clipboard\n\
        Esc          : Return to Normal Mode\n\
        \n\
        [ App ]\n\
        :q / q       : Quit\n\
        ";
        let help_paragraph = Paragraph::new(help_text).block(
            Block::default().title(" Help Menu ").borders(Borders::ALL).border_type(BorderType::Double).style(Style::default().bg(Color::Reset).fg(Color::Cyan))
        );
        f.render_widget(Clear, area);
        f.render_widget(help_paragraph, area);
    }
    
    // Popup logic refactoring helper
    let render_prompt = |f: &mut Frame, title: &str, prefix: &str, buffer: &str| {
        let v_margin = (f.area().height.saturating_sub(3)) / 2;
        let area_v = Layout::default().direction(Direction::Vertical).constraints([
            Constraint::Length(v_margin), Constraint::Length(3), Constraint::Min(0)]).split(f.area())[1];
        let h_margin = (f.area().width.saturating_sub(40)) / 2;
        let area = Layout::default().direction(Direction::Horizontal).constraints([
            Constraint::Length(h_margin), Constraint::Length(40), Constraint::Min(0)]).split(area_v)[1];
            
        let p_text = format!("{} {}", prefix, buffer);
        let block = Block::default()
            .title(title).borders(Borders::ALL).border_type(BorderType::Double)
            .style(Style::default().bg(Color::Reset).fg(Color::Magenta));
            
        let paragraph = Paragraph::new(p_text.as_str()).block(block).style(Style::default().fg(Color::White));
        
        f.render_widget(Clear, area);
        f.render_widget(paragraph, area);
        let prefix_len = prefix.chars().count() as u16 + 1;
        f.set_cursor_position((area.x + 1 + prefix_len + buffer.len() as u16, area.y + 1));
    };

    if app.mode == AppMode::PromptNewFile {
        render_prompt(f, " Create New File ", "Filename (*.md):", &app.prompt_buffer);
    } else if app.mode == AppMode::PromptRenameFile {
        render_prompt(f, " Rename File ", "New Name (*.md):", &app.prompt_buffer);
    } else if app.mode == AppMode::SearchFile {
        render_prompt(f, " List Filter ", "Search:", &app.prompt_buffer);
    }
}
