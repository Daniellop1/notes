use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use magic_crypt::{new_magic_crypt, MagicCryptTrait};
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
    ShowStats,
    PromptNewFile,
    PromptRenameFile,
    PromptVaultPassword,
    SearchFile,
    Visual,
}

// --- Dynamic Theming ---
#[derive(Clone, Copy)]
struct Theme {
    top_bg: Color,
    top_fg: Color,
    border: Color,
    border_focused: Color,
    status_normal: Color,
    status_insert: Color,
    status_command: Color,
    status_visual: Color,
    status_other: Color,
    text_normal: Color,
    text_heading: Color,
    text_quote: Color,
    text_bullet: Color,
    text_search: Color,
    text_search_bg: Color,
}

impl Theme {
    fn default_theme() -> Self {
        Self {
            top_bg: Color::Blue,
            top_fg: Color::Black,
            border: Color::Reset,
            border_focused: Color::Reset,
            status_normal: Color::Blue,
            status_insert: Color::Green,
            status_command: Color::Yellow,
            status_visual: Color::Rgb(255, 165, 0), // Orange
            status_other: Color::Magenta,
            text_normal: Color::White,
            text_heading: Color::LightBlue,
            text_quote: Color::Cyan,
            text_bullet: Color::Yellow,
            text_search: Color::Black,
            text_search_bg: Color::Yellow,
        }
    }

    fn hacker_theme() -> Self {
        Self {
            top_bg: Color::Green,
            top_fg: Color::Black,
            border: Color::DarkGray,
            border_focused: Color::Green,
            status_normal: Color::Green,
            status_insert: Color::LightGreen,
            status_command: Color::DarkGray,
            status_visual: Color::Yellow,
            status_other: Color::DarkGray,
            text_normal: Color::Green,
            text_heading: Color::LightGreen,
            text_quote: Color::DarkGray,
            text_bullet: Color::Green,
            text_search: Color::Black,
            text_search_bg: Color::LightGreen,
        }
    }

    fn ocean_theme() -> Self {
        Self {
            top_bg: Color::Cyan,
            top_fg: Color::Black,
            border: Color::Blue,
            border_focused: Color::Cyan,
            status_normal: Color::Cyan,
            status_insert: Color::LightCyan,
            status_command: Color::LightBlue,
            status_visual: Color::Magenta,
            status_other: Color::Blue,
            text_normal: Color::Cyan,
            text_heading: Color::LightCyan,
            text_quote: Color::Blue,
            text_bullet: Color::Blue,
            text_search: Color::Black,
            text_search_bg: Color::White,
        }
    }

    fn light_theme() -> Self {
        Self {
            top_bg: Color::White,
            top_fg: Color::Black,
            border: Color::Black,
            border_focused: Color::Black,
            status_normal: Color::White,
            status_insert: Color::Rgb(220, 220, 220),
            status_command: Color::Rgb(150, 150, 150),
            status_visual: Color::Yellow,
            status_other: Color::DarkGray,
            text_normal: Color::White, 
            text_heading: Color::White,
            text_quote: Color::Rgb(150, 150, 150),
            text_bullet: Color::White,
            text_search: Color::Black,
            text_search_bg: Color::White,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum EncryptBits {
    B64,
    B128,
    B192,
    B256,
}

impl EncryptBits {
    fn label(&self) -> &'static str {
        match self {
            EncryptBits::B64  => "DES-64",
            EncryptBits::B128 => "AES-128",
            EncryptBits::B192 => "AES-192",
            EncryptBits::B256 => "AES-256",
        }
    }
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

    // Epic Toggle Features
    is_zen_mode: bool,
    theme: Theme,
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,

    // Vault Encryption
    master_password: String,
    vault_active: bool,
    encrypt_bits: EncryptBits,

    // UI State
    is_modified: bool,
    pending_quit: bool,
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
            is_zen_mode: false,
            theme: Theme::default_theme(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            master_password: String::new(),
            vault_active: false,
            encrypt_bits: EncryptBits::B256,
            is_modified: false,
            pending_quit: false,
        };

        app.refresh_files();
        if !app.files.is_empty() {
            app.file_list_state.select(Some(0));
            app.load_file();
        }

        app
    }

    fn set_theme(&mut self, theme_name: &str) {
        self.theme = match theme_name.trim() {
            "hacker" => Theme::hacker_theme(),
            "ocean" => Theme::ocean_theme(),
            "light" => Theme::light_theme(),
            _ => Theme::default_theme(),
        };
        self.status_message = format!("Theme changed to '{}'", theme_name);
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
                    let raw = fs::read_to_string(&path).unwrap_or_default();
                    self.content = self.decrypt_content(&raw);
                    self.cursor_index = 0; 
                    self.editor_scroll_y = 0;
                    self.editor_scroll_x = 0;
                    self.visual_anchor = None;
                    self.undo_stack.clear();
                    self.redo_stack.clear();
                    self.is_modified = false;
                    self.status_message = format!("Loaded: {}", path.file_name().unwrap_or_default().to_string_lossy());
                }
            }
        }
    }

    fn save_file(&mut self) {
        if let Some(path) = &self.active_file {
            let data = self.encrypt_content(&self.content.clone());
            if let Err(e) = fs::write(path, &data) {
                self.status_message = format!("Error saving: {}", e);
            } else {
                let lock_mark = if self.vault_active {
                    format!(" 🔒 {}", self.encrypt_bits.label())
                } else {
                    String::new()
                };
                self.status_message = format!("Saved{}", lock_mark);
                self.is_modified = false;
                self.pending_quit = false;
            }
        }
    }

    // --- Vault Encryption Engine (Self-Describing Format) ---
    //
    // Encrypted files are saved with a prefix: "VAULT:256:\n" + base64
    // This means ANY file can be identified as encrypted without app state.
    // On load, if the header is detected and vault is active, we decrypt.
    // If header is detected but vault is OFF, we show a "locked" indicator.

    fn make_vault_header(&self) -> String {
        let bits: u16 = match self.encrypt_bits {
            EncryptBits::B64  => 64,
            EncryptBits::B128 => 128,
            EncryptBits::B192 => 192,
            EncryptBits::B256 => 256,
        };
        format!("VAULT:{}:\n", bits)
    }

    fn encrypt_content(&self, content: &str) -> String {
        if self.vault_active && !self.master_password.is_empty() {
            let header = self.make_vault_header();
            let encrypted = match self.encrypt_bits {
                EncryptBits::B64  => { let mc = new_magic_crypt!(&self.master_password, 64);  mc.encrypt_str_to_base64(content) }
                EncryptBits::B128 => { let mc = new_magic_crypt!(&self.master_password, 128); mc.encrypt_str_to_base64(content) }
                EncryptBits::B192 => { let mc = new_magic_crypt!(&self.master_password, 192); mc.encrypt_str_to_base64(content) }
                EncryptBits::B256 => { let mc = new_magic_crypt!(&self.master_password, 256); mc.encrypt_str_to_base64(content) }
            };
            format!("{}{}", header, encrypted)
        } else {
            content.to_string()
        }
    }

    /// Detects if raw file bytes have a vault header.
    /// Returns the (bits, base64_payload) if encrypted, or None if plaintext.
    fn parse_vault_header(raw: &str) -> Option<(u16, &str)> {
        if raw.starts_with("VAULT:") {
            let rest = &raw[6..];
            if let Some(colon_pos) = rest.find(':') {
                let bits_str = &rest[..colon_pos];
                // payload is after bits + ":\n"
                let payload = &rest[colon_pos + 2..];
                if let Ok(bits) = bits_str.parse::<u16>() {
                    return Some((bits, payload));
                }
            }
        }
        None
    }

    fn decrypt_content(&self, raw: &str) -> String {
        match Self::parse_vault_header(raw) {
            Some((bits, payload)) => {
                // File is encrypted. Only attempt decryption if vault is active + password set.
                if self.master_password.is_empty() {
                    // Return a user-friendly placeholder
                    return format!("[LOCKED - {} encrypted]\n\nType :vault to unlock this note.", 
                        match bits { 64 => "DES-64", 128 => "AES-128", 192 => "AES-192", _ => "AES-256" });
                }
                let result = match bits {
                    64  => { let mc = new_magic_crypt!(&self.master_password, 64);  mc.decrypt_base64_to_string(payload) }
                    128 => { let mc = new_magic_crypt!(&self.master_password, 128); mc.decrypt_base64_to_string(payload) }
                    192 => { let mc = new_magic_crypt!(&self.master_password, 192); mc.decrypt_base64_to_string(payload) }
                    _   => { let mc = new_magic_crypt!(&self.master_password, 256); mc.decrypt_base64_to_string(payload) }
                };
                match result {
                    Ok(plain) => plain,
                    Err(_) => "[WRONG PASSWORD - decryption failed]\n\nType :vault to try a different password.".to_string(),
                }
            }
            None => raw.to_string(), // Plaintext file, return as-is
        }
    }

    fn apply_vault_password(&mut self) {
        let password = self.prompt_buffer.clone();
        self.prompt_buffer.clear();
        self.mode = AppMode::Normal;

        if password.is_empty() {
            // Disable vault — save current file as plaintext if it's in buffer
            self.vault_active = false;
            self.master_password.clear();
            self.status_message = "Vault disabled. New saves will be plaintext.".into();
            // Re-load the current file to show it (decrypted state stays)
            return;
        }

        self.master_password = password;
        self.vault_active = true;

        // Re-read the active file from disk and attempt decryption with new password
        if let Some(path) = self.active_file.clone() {
            let raw = fs::read_to_string(&path).unwrap_or_default();
            let decoded = self.decrypt_content(&raw);
            self.content = decoded;
            self.cursor_index = self.cursor_index.min(self.content.len());

            let algo = match self.encrypt_bits {
                EncryptBits::B64  => "DES-64",
                EncryptBits::B128 => "AES-128",
                EncryptBits::B192 => "AES-192",
                EncryptBits::B256 => "AES-256",
            };

            if self.content.starts_with("[WRONG PASSWORD") {
                self.status_message = "Wrong password! File still locked.".into();
                self.vault_active = false;
                self.master_password.clear();
            } else {
                self.status_message = format!("Vault unlocked ({}) - saves will be encrypted. 🔒", algo);
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
    
    fn toggle_task_checkbox(&mut self) {
        let (_, row) = self.get_visual_cursor();
        let lines: Vec<&str> = self.content.split('\n').collect();
        if let Some(line) = lines.get(row as usize) {
            let mut changed = false;
            let mut rebuilt: Vec<String> = lines.iter().map(|&s| s.to_string()).collect();
            let trimmed = line.trim_start();
            if trimmed.starts_with("- [ ] ") {
                rebuilt[row as usize] = line.replacen("- [ ] ", "- [x] ", 1);
                changed = true;
            } else if trimmed.starts_with("- [x] ") {
                rebuilt[row as usize] = line.replacen("- [x] ", "- [ ] ", 1);
                changed = true;
            }
            if changed {
                self.save_undo_state();
                self.content = rebuilt.join("\n");
                self.save_file();
                self.status_message = "Task state toggled.".into();
            }
        }
    }

    // --- Cursor Tracking & Scroll Updates ---
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
        self.is_modified = true;
    }

    fn insert_string(&mut self, s: &str) {
        self.save_undo_state();
        self.content.insert_str(self.cursor_index, s);
        self.cursor_index += s.len();
        self.save_file();
    }

    fn delete_char(&mut self) {
        if self.cursor_index > 0 {
            let mut index = self.cursor_index - 1;
            while !self.content.is_char_boundary(index) { index -= 1; }
            self.content.remove(index);
            self.cursor_index = index;
            self.is_modified = true;
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
            self.save_undo_state();
            self.content.drain(start..end);
            self.cursor_index = start;
            self.mode = AppMode::Normal;
            self.visual_anchor = None;
            self.is_modified = true;
            self.status_message = format!("Cut {} bytes.", self.clipboard.len());
            self.save_file();
        }
    }

    // --- Undo / Redo System ---
    fn save_undo_state(&mut self) {
        if let Some(last) = self.undo_stack.last() {
            if last == &self.content {
                return; // Suppress identical consecutive states
            }
        }
        self.undo_stack.push(self.content.clone());
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn perform_undo(&mut self) {
        if let Some(prev_state) = self.undo_stack.pop() {
            self.redo_stack.push(self.content.clone());
            self.content = prev_state;
            self.cursor_index = self.cursor_index.min(self.content.len());
            self.save_file();
            self.status_message = "Undo".into();
        } else {
            self.status_message = "Already at oldest change".into();
        }
    }

    fn perform_redo(&mut self) {
        if let Some(next_state) = self.redo_stack.pop() {
            self.undo_stack.push(self.content.clone());
            self.content = next_state;
            self.cursor_index = self.cursor_index.min(self.content.len());
            self.save_file();
            self.status_message = "Redo".into();
        } else {
            self.status_message = "Already at newest change".into();
        }
    }
}

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
                                if app.is_modified && !app.pending_quit {
                                    app.pending_quit = true;
                                    app.status_message = "Unsaved changes! Press 'q' again to quit, or ':w' to save.".into();
                                } else {
                                    app.save_file();
                                    return Ok(());
                                }
                            }
                            (KeyCode::Char('u'), _) => app.perform_undo(),
                            (KeyCode::Char('r'), KeyModifiers::CONTROL) => app.perform_redo(),
                            // Epic Focus Bindings
                            (KeyCode::Char('z'), _) => {
                                app.is_zen_mode = !app.is_zen_mode;
                                app.status_message = if app.is_zen_mode { "Zen Mode ON".into() } else { "Zen Mode OFF".into() };
                            }
                            (KeyCode::Char('i'), KeyModifiers::CONTROL) => {
                                app.mode = AppMode::ShowStats;
                            }
                            (KeyCode::Char(' '), _) => {
                                app.toggle_task_checkbox();
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
                                    app.save_undo_state();
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
                                } else if app.command_buffer.starts_with(":theme ") {
                                    let theme_name = app.command_buffer.replace(":theme ", "");
                                    app.set_theme(&theme_name);
                                } else if app.command_buffer.starts_with(":vault") {
                                    // Parse optional bit strength: :vault, :vault 64, :vault 128, :vault 192, :vault 256
                                    let arg = app.command_buffer.trim_start_matches(":vault").trim();
                                    app.encrypt_bits = match arg {
                                        "64"  => EncryptBits::B64,
                                        "128" => EncryptBits::B128,
                                        "192" => EncryptBits::B192,
                                        _     => EncryptBits::B256, // default
                                    };
                                    app.mode = AppMode::PromptVaultPassword;
                                    app.prompt_buffer.clear();
                                    app.command_buffer.clear();
                                    continue;
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
                        AppMode::ShowHelp | AppMode::ShowStats => match key.code {
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
                        },
                        AppMode::PromptVaultPassword => match key.code {
                            KeyCode::Esc => {
                                app.mode = AppMode::Normal;
                                app.prompt_buffer.clear();
                                app.status_message = "Vault prompt cancelled.".into();
                            }
                            KeyCode::Enter => app.apply_vault_password(),
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

fn generate_highlighted_lines<'a>(app: &'a App) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    let mut byte_offset = 0;
    
    let (sel_min, sel_max) = match app.visual_anchor {
        Some(anchor) => (anchor.min(app.cursor_index), anchor.max(app.cursor_index)),
        None => (0, 0),
    };
    let is_visual = app.visual_anchor.is_some();

    for line_str in app.content.split('\n') {
        let line_len = line_str.len();
        let line_start = byte_offset;
        let line_end = byte_offset + line_len; 
        
        let mut base_style = Style::default().fg(app.theme.text_normal);
        
        if line_str.starts_with("# ") || line_str.starts_with("## ") || line_str.starts_with("### ") {
            base_style = base_style.fg(app.theme.text_heading).add_modifier(Modifier::BOLD);
        } else if line_str.starts_with("> ") {
            base_style = base_style.fg(app.theme.text_quote).add_modifier(Modifier::ITALIC);
        } else if line_str.starts_with("- ") || line_str.starts_with("* ") {
            base_style = base_style.fg(app.theme.text_bullet);
        }

        if !is_visual || sel_max <= line_start || sel_min >= line_end {
            if !app.search_query.is_empty() && line_str.to_lowercase().contains(&app.search_query.to_lowercase()) {
                let lower = line_str.to_lowercase();
                let q = app.search_query.to_lowercase();
                let mut ptr = 0;
                let mut spans = Vec::new();
                
                for (idx, _) in lower.match_indices(&q) {
                    if idx > ptr {
                        spans.push(Span::styled(line_str[ptr..idx].to_string(), base_style));
                    }
                    spans.push(Span::styled(
                        line_str[idx..idx + q.len()].to_string(), 
                        base_style.bg(app.theme.text_search_bg).fg(app.theme.text_search).add_modifier(Modifier::BOLD)
                    ));
                    ptr = idx + q.len();
                }
                if ptr < line_len {
                    spans.push(Span::styled(line_str[ptr..].to_string(), base_style));
                }
                lines.push(Line::from(spans));
            } else {
                lines.push(Line::from(Span::styled(line_str.to_string(), base_style)));
            }
        } else {
            let start_idx = sel_min.saturating_sub(line_start).min(line_len);
            let end_idx = sel_max.saturating_sub(line_start).min(line_len);

            let mut spans = Vec::new();
            if start_idx > 0 {
                spans.push(Span::styled(line_str[..start_idx].to_string(), base_style));
            }
            if start_idx < end_idx {
                spans.push(Span::styled(
                    line_str[start_idx..end_idx].to_string(), 
                    // Using visual standard regardless of theme for safety, or we could add generic overlay.
                    base_style.bg(Color::DarkGray) 
                ));
            }
            if end_idx < line_len {
                spans.push(Span::styled(line_str[end_idx..].to_string(), base_style));
            }
            lines.push(Line::from(spans));
        }

        byte_offset += line_len + 1;
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

    // Compose top bar with vault status aligned right
    let vault_label = if app.vault_active {
        format!(" 🔒 {} ", app.encrypt_bits.label())
    } else {
        " No Vault ".to_string()
    };
    let total_width = f.area().width as usize;
    let title_str = "NOTES";
    let padding = total_width.saturating_sub(title_str.len() + vault_label.len() + 1);
    let top_text = format!("{:^width$}{}", title_str, vault_label, width = padding + title_str.len());

    let top_bar = Paragraph::new(top_text)
        .style(Style::default().bg(app.theme.top_bg).fg(app.theme.top_fg).add_modifier(Modifier::BOLD));
    f.render_widget(top_bar, chunks[0]);

    // Apply Zen Mode Layout
    let (left_pane_width, right_pane_width) = if app.is_zen_mode {
        (0, 100)
    } else {
        (25, 75)
    };

    let main_panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(left_pane_width), Constraint::Percentage(right_pane_width)])
        .split(chunks[1]);

    // --- Left Pane: File Explorer ---
    if !app.is_zen_mode {
        let note_count = app.files.len();
        let active_path = app.active_file.clone();

        let items: Vec<ListItem> = app.files.iter().map(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            let size_str = std::fs::metadata(p).map(|m| {
                let b = m.len();
                if b >= 1024 { format!("{} KB", b / 1024) } else { format!("{} B", b) }
            }).unwrap_or_default();
            let lock = if app.vault_active && active_path.as_ref() == Some(p) { "🔒 " } else { "" };
            let label = format!("{}{:<14} {:>6}", lock, 
                if name.len() > 14 { format!("{}…", &name[..13]) } else { name.clone() },
                size_str);
            ListItem::new(label)
        }).collect();

        let title = if app.mode == AppMode::Normal {
            format!(" [Files] ({}) ", note_count)
        } else {
            format!(" Files ({}) ", note_count)
        };
        
        let border_color = if app.mode == AppMode::Normal { app.theme.border_focused } else { app.theme.border };
        
        let list = List::new(items)
            .block(Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .style(Style::default().fg(border_color))
            )
            .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");
        f.render_stateful_widget(list, main_panes[0], &mut app.file_list_state);
    }

    // --- Right Pane: Editor & Scrollbar ---
    let modified_mark = if app.is_modified { " [+]" } else { "" };
    let e_title = match app.mode {
        AppMode::Insert => format!(" [Editor (INSERT)]{} ", modified_mark),
        AppMode::Visual => format!(" [Editor (VISUAL)]{} ", modified_mark),
        _ => if app.is_zen_mode {
            format!(" Focus Mode{} ", modified_mark)
        } else {
            format!(" Editor{} ", modified_mark)
        },
    };
    
    let editor_border_color = match app.mode {
        AppMode::Insert | AppMode::Visual => app.theme.border_focused,
        _ => if app.is_modified { Color::Yellow } else { app.theme.border },
    };
    
    let editor_block = Block::default()
        .title(e_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .style(Style::default().fg(editor_border_color));

    let inner_editor_area = editor_block.inner(main_panes[1]);
    
    if app.mode == AppMode::Insert || app.mode == AppMode::Visual {
        app.update_scroll_to_cursor(inner_editor_area.height, inner_editor_area.width);
    }
    
    let doc_lines = if app.active_file.is_some() {
        generate_highlighted_lines(app)
    } else {
        vec![Line::from(Span::styled("No note selected. Press 'n' to create one.", Style::default().fg(app.theme.text_normal)))]
    };
    
    let total_lines = doc_lines.len();

    let paragraph = Paragraph::new(doc_lines)
        .block(editor_block)
        .scroll((app.editor_scroll_y, app.editor_scroll_x)); 

    f.render_widget(paragraph, main_panes[1]);

    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("▲"))
        .end_symbol(Some("▼"));
        
    let mut scrollbar_state = ScrollbarState::default()
        .content_length(total_lines)
        .position(app.editor_scroll_y as usize);
        
    f.render_stateful_widget(scrollbar, main_panes[1], &mut scrollbar_state);

    if (app.mode == AppMode::Insert || app.mode == AppMode::Visual) && app.active_file.is_some() {
        let (col, row) = app.get_visual_cursor();
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
        AppMode::ShowStats => " STATS ",
        AppMode::PromptNewFile => " PROMPT ",
        AppMode::PromptRenameFile => " RENAME ",
        AppMode::PromptVaultPassword => " VAULT ",
        AppMode::SearchFile => " SEARCH ",
        AppMode::Visual => " VISUAL ",
    };

    let file_info = match &app.active_file {
        Some(p) => p.file_name().unwrap_or_default().to_string_lossy(),
        None => "[No File]".into(),
    };

    let color = match app.mode {
        AppMode::Normal  => app.theme.status_normal,
        AppMode::Insert  => app.theme.status_insert,
        AppMode::Command => app.theme.status_command,
        AppMode::Visual  => app.theme.status_visual,
        _ => app.theme.status_other,
    };

    let status_text = match app.mode {
        AppMode::Command => format!("{}  {}", mode_str, app.command_buffer),
        _ => {
            let (col, row) = app.get_visual_cursor();
            let ln_col = format!("Ln {}, Col {}", row + 1, col + 1);
            let modified_str = if app.is_modified { " [+]" } else { "" };
            let filter_str = if !app.search_query.is_empty() { "[Filtered] " } else { "" };
            format!("{}{}{} | File: {} | {} | {}",
                mode_str, filter_str, modified_str, file_info, ln_col, app.status_message)
        }
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
        z            : Toggle Focus (Zen) Mode\n\
        Space        : Toggle Checkbox (- [ ])\n\
        Ctrl+I       : Document Statistics\n\
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
        u            : Undo previous text change\n\
        Ctrl+r       : Redo text change\n\
        \n\
        [ App ]\n\
        :theme [x]   : Set theme (hacker/ocean/light/default)\n\
        :vault       : Set/change encryption password\n\
        :q / q       : Quit\n\
        ";
        let help_paragraph = Paragraph::new(help_text).block(
            Block::default().title(" Help Menu ").borders(Borders::ALL).border_type(BorderType::Double).style(Style::default().bg(Color::Reset).fg(app.theme.status_other))
        );
        f.render_widget(Clear, area);
        f.render_widget(help_paragraph, area);
    }
    
    if app.mode == AppMode::ShowStats {
        let area = centered_rect(40, 30, f.area());
        
        let word_count = app.content.split_whitespace().count();
        let char_count = app.content.chars().count();
        let bytes = app.content.len();
        let read_time = (word_count as f32 / 200.0).ceil() as usize;

        let stats_text = format!("\n\
        File Size: {} bytes\n\n\
        Word Count: {}\n\
        Character Count: {}\n\n\
        Estimated Read Time: ~{} min\n\
        ", bytes, word_count, char_count, read_time);

        let stats_paragraph = Paragraph::new(stats_text)
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::default().title(" Document Stats ").borders(Borders::ALL).border_type(BorderType::Double).style(Style::default().bg(Color::Reset).fg(app.theme.status_other))
            );
        f.render_widget(Clear, area);
        f.render_widget(stats_paragraph, area);
    }
    
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
            .style(Style::default().bg(Color::Reset).fg(app.theme.status_other));
            
        let paragraph = Paragraph::new(p_text.as_str()).block(block).style(Style::default().fg(Color::Reset)); // Changed to Reset so light theme reads correctly
        
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
    } else if app.mode == AppMode::PromptVaultPassword {
        // Render vault prompt with masked password (stars instead of characters)
        let v_margin = (f.area().height.saturating_sub(3)) / 2;
        let area_v = Layout::default().direction(Direction::Vertical).constraints([
            Constraint::Length(v_margin), Constraint::Length(5), Constraint::Min(0)]).split(f.area())[1];
        let h_margin = (f.area().width.saturating_sub(44)) / 2;
        let area = Layout::default().direction(Direction::Horizontal).constraints([
            Constraint::Length(h_margin), Constraint::Length(44), Constraint::Min(0)]).split(area_v)[1];

        let masked = "*".repeat(app.prompt_buffer.len());
        let vault_status = if app.vault_active { " [ACTIVE 🔒] " } else { " [OFF] " };
        let p_text = format!("Password{}: {}", vault_status, masked);
        let hint = "\n(Enter to confirm, Esc to cancel, empty=disable)";
        let full_text = format!("{}{}", p_text, hint);

        let block = Block::default()
            .title(" 🔐 Vault Encryption ")
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .style(Style::default().bg(Color::Reset).fg(Color::Magenta));

        let paragraph = Paragraph::new(full_text.as_str())
            .block(block)
            .style(Style::default().fg(Color::White));

        f.render_widget(Clear, area);
        f.render_widget(paragraph, area);
        // Place cursor at end of masked input
        let prefix_len = format!("Password{}:  ", vault_status).chars().count() as u16;
        f.set_cursor_position((area.x + 1 + prefix_len + masked.len() as u16, area.y + 1));
    }
}
