use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
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
}

struct App {
    mode: AppMode,
    notes_dir: PathBuf,
    files: Vec<PathBuf>,
    file_list_state: ListState,
    active_file: Option<PathBuf>,
    content: String,
    cursor_index: usize,
    command_buffer: String,
    prompt_buffer: String,
    status_message: String,
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
            command_buffer: String::new(),
            prompt_buffer: String::new(),
            status_message: String::new(),
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
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "md") {
                    self.files.push(path);
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
                // Determine if we're actually changing files
                let needs_load = match &self.active_file {
                    Some(active) => active != &path,
                    None => true,
                };

                if needs_load {
                    // Auto-save currently active file before replacing buffer
                    if self.active_file.is_some() {
                        self.save_file();
                    }
                    self.active_file = Some(path.clone());
                    self.content = fs::read_to_string(&path).unwrap_or_default();
                    self.cursor_index = self.content.len();
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
                self.status_message = String::from("Saved successfully.");
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
            self.status_message = "File creation cancelled.".into();
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
            self.status_message = format!("Error creating file: {}", e);
        } else {
            // Auto-save current
            if self.active_file.is_some() {
                self.save_file();
            }
            
            self.refresh_files();
            
            // Find the new file in the sorted list and select it
            let new_index = self.files.iter().position(|p| p == &path).unwrap_or(0);
            self.file_list_state.select(Some(new_index));
            self.active_file = Some(path.clone());
            self.content.clear();
            self.cursor_index = 0;

            self.status_message = format!("Created {}", filename);
            self.mode = AppMode::Insert; // Jump right into typing
        }
        self.prompt_buffer.clear();
    }

    fn next_file(&mut self) {
        if self.files.is_empty() { return; }
        
        let i = match self.file_list_state.selected() {
            Some(i) => if i >= self.files.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.file_list_state.select(Some(i));
        self.load_file(); // Implicitly triggers auto-save via load_file logic
    }

    fn previous_file(&mut self) {
        if self.files.is_empty() { return; }
        let i = match self.file_list_state.selected() {
            Some(i) => if i == 0 { self.files.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.file_list_state.select(Some(i));
        self.load_file(); // Auto-saves if changing
    }

    // --- Insert Mode Helpers ---
    fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor_index, c);
        self.cursor_index += c.len_utf8();
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
    fn get_visual_cursor(&self) -> (u16, u16) {
        let text_before_cursor = &self.content[..self.cursor_index];
        let lines: Vec<&str> = text_before_cursor.split('\n').collect();
        let row = (lines.len() as u16).saturating_sub(1);
        let col = lines.last().map(|l| l.chars().count()).unwrap_or(0) as u16;
        (col, row)
    }
}

// UI Helper functionality
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
                            (KeyCode::Char('j'), _) | (KeyCode::Down, _) => app.next_file(),
                            (KeyCode::Char('k'), _) | (KeyCode::Up, _) => app.previous_file(),
                            (KeyCode::Enter, _) => app.load_file(),
                            (KeyCode::Char('n'), _) => {
                                app.mode = AppMode::PromptNewFile;
                                app.prompt_buffer.clear();
                            }
                            (KeyCode::Char('?'), _) | (KeyCode::F(1), _) => {
                                app.mode = AppMode::ShowHelp;
                            }
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
                            (KeyCode::Char('s'), KeyModifiers::CONTROL) => app.save_file(),
                            _ => {}
                        },
                        AppMode::Insert => match (key.code, key.modifiers) {
                            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                                app.save_file();
                                return Ok(());
                            }
                            (KeyCode::Esc, _) => {
                                app.mode = AppMode::Normal;
                                app.status_message = "Returned to Normal Mode".into();
                            }
                            (KeyCode::Char('s'), KeyModifiers::CONTROL) => app.save_file(),
                            (KeyCode::Char(c), _) => app.insert_char(c),
                            (KeyCode::Enter, _) => app.insert_char('\n'),
                            (KeyCode::Backspace, _) => app.delete_char(),
                            (KeyCode::Left, _) => app.move_cursor_left(),
                            (KeyCode::Right, _) => app.move_cursor_right(),
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
                            _ => app.mode = AppMode::Normal, // Any key closes help
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
                        }
                    }
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top bar
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Bottom status bar
        ])
        .split(f.area());

    // --- Top Bar ---
    let top_bar = Paragraph::new(" NOTES ")
        .style(Style::default().bg(Color::Blue).fg(Color::Black).add_modifier(Modifier::BOLD))
        // Center alignment if available, but a space padded string works for just aesthetics
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

    // --- Right Pane: Editor ---
    let e_title = if app.mode == AppMode::Insert { " [Editor (INSERT)] " } else { " Editor " };
    let editor_block = Block::default()
        .title(e_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Thick);

    let doc_content = if app.active_file.is_some() {
        app.content.as_str()
    } else {
        "No note selected. Press 'n' to create one."
    };

    let paragraph = Paragraph::new(doc_content)
        .block(editor_block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, main_panes[1]);

    // Handle cursor in Insert mode
    if app.mode == AppMode::Insert && app.active_file.is_some() {
        let (col, row) = app.get_visual_cursor();
        f.set_cursor_position((
            main_panes[1].x + 1 + col,
            main_panes[1].y + 1 + row,
        ));
    }

    // --- Bottom Bar: Status Bar ---
    let mode_str = match app.mode {
        AppMode::Normal => " NORMAL ",
        AppMode::Insert => " INSERT ",
        AppMode::Command => " COMMAND ",
        AppMode::ShowHelp => " HELP ",
        AppMode::PromptNewFile => " PROMPT ",
    };

    let file_info = match &app.active_file {
        Some(p) => p.file_name().unwrap_or_default().to_string_lossy(),
        None => "[No File]".into(),
    };

    let status_text = match app.mode {
        AppMode::Command => format!("{}  {}", mode_str, app.command_buffer),
        _ => format!("{} | File: {} | (? for Help) | {}", mode_str, file_info, app.status_message),
    };

    let color = match app.mode {
        AppMode::Normal => Color::Blue,
        AppMode::Insert => Color::Green,
        AppMode::Command => Color::Yellow,
        AppMode::ShowHelp => Color::Cyan,
        AppMode::PromptNewFile => Color::Magenta,
    };

    let status_bar = Paragraph::new(status_text)
        .style(Style::default().bg(color).fg(Color::Black).add_modifier(Modifier::BOLD));

    f.render_widget(status_bar, chunks[2]);

    // --- Overlays ---
    
    // Help Overlay
    if app.mode == AppMode::ShowHelp {
        let area = centered_rect(50, 50, f.area());
        let help_text = "\
        [ Navigation Controls ]\n\
        j / Down  : Move down file list\n\
        k / Up    : Move up file list\n\
        Enter     : Open highlighted file\n\
        \n\
        [ Editor Controls ]\n\
        i         : Enter Insert Mode\n\
        Esc       : Return to Normal Mode\n\
        \n\
        [ File Operations ]\n\
        n         : Create new file\n\
        d         : Delete active file\n\
        :w / C-s  : Save file\n\
        \n\
        [ Application ]\n\
        :q / q    : Quit\n\
        ? / F1    : Show Help\n\
        ";
        
        let block = Block::default()
            .title(" Help Menu (Press any key to close) ")
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .style(Style::default().bg(Color::Reset).fg(Color::Cyan));
            
        let help_paragraph = Paragraph::new(help_text).block(block);
        
        f.render_widget(Clear, area); // Clear background
        f.render_widget(help_paragraph, area);
    }
    
        // Prompt Overlay
    if app.mode == AppMode::PromptNewFile {
        // Use an absolute height of 3 (1 top border, 1 text line, 1 bottom border)
        // to ensure the prompt is always completely visible regardless of terminal height.
        let v_margin = (f.area().height.saturating_sub(3)) / 2;
        let area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(v_margin),
                Constraint::Length(3), // Exact height needed for standard prompt
                Constraint::Min(0),
            ])
            .split(f.area())[1];

        let h_margin = (f.area().width.saturating_sub(40)) / 2;
        let area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(h_margin),
                Constraint::Length(40), // Exact width
                Constraint::Min(0),
            ])
            .split(area)[1];

        let p_text = format!("Filename (*.md): {}", app.prompt_buffer);
        
        let block = Block::default()
            .title(" Create New File ")
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .style(Style::default().bg(Color::Reset).fg(Color::Magenta));
            
        // Explicitly set the text style here so it's readable
        let prompt_paragraph = Paragraph::new(p_text.as_str())
            .block(block)
            .style(Style::default().fg(Color::White));
        
        f.render_widget(Clear, area);
        f.render_widget(prompt_paragraph, area);
        
        f.set_cursor_position((
            area.x + 18 + app.prompt_buffer.len() as u16,
            area.y + 1,
        ));
    }
}
