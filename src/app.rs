use std::collections::HashSet;
use std::{fs, path::PathBuf};
use ratatui::widgets::ListState;
use crate::theme::Theme;
use crate::vault::{EncryptBits, Vault};
use crate::config::Config;

#[derive(PartialEq)]
pub enum AppMode {
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

pub struct App {
    pub mode: AppMode,
    pub notes_dir: PathBuf,
    pub config: Config,
    pub files: Vec<PathBuf>,
    pub pending_g: bool,
    pub file_list_state: ListState,
    pub active_file: Option<PathBuf>,
    pub content: String,
    pub cursor_index: usize,
    pub visual_anchor: Option<usize>,
    pub clipboard: String,
    pub command_buffer: String,
    pub prompt_buffer: String,
    pub search_query: String,
    pub status_message: String,
    pub editor_scroll_y: u16,
    pub editor_scroll_x: u16,

    // Epic Toggle Features
    pub is_zen_mode: bool,
    pub theme: Theme,
    pub undo_stack: Vec<String>,
    pub redo_stack: Vec<String>,

    // Vault Encryption
    pub master_password: String,
    pub vault_active: bool,
    pub encrypt_bits: EncryptBits,

    // UI State
    pub is_modified: bool,
    pub pending_quit: bool,
    pub pending_delete: bool,
    pub active_file_is_encrypted: bool,
    pub encrypted_files: HashSet<PathBuf>,
}

impl App {
    pub fn new() -> Self {
        let config = Config::load_or_default();
        let notes_dir = config.notes_dir.clone();

        if !notes_dir.exists() {
            let _ = fs::create_dir_all(&notes_dir);
        }

        let mut app = Self {
            mode: AppMode::Normal,
            notes_dir,
            config,
            files: Vec::new(),
            pending_g: false,
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
            pending_delete: false,
            active_file_is_encrypted: false,
            encrypted_files: HashSet::new(),
        };

        // Apply saved theme from config
        let theme_str = app.config.theme.clone();
        app.set_theme(&theme_str);

        app.refresh_files();
        if !app.files.is_empty() {
            app.file_list_state.select(Some(0));
            app.load_file();
        }

        app
    }

    pub fn set_theme(&mut self, theme_name: &str) {
        self.theme = match theme_name.trim() {
            "hacker" => Theme::hacker_theme(),
            "ocean" => Theme::ocean_theme(),
            "light" => Theme::light_theme(),
            _ => Theme::default_theme(),
        };
        self.config.theme = theme_name.to_string();
        let _ = self.config.save();
        self.status_message = format!("Theme changed to '{}' (saved)", theme_name);
    }

    pub fn refresh_files(&mut self) {
        self.files.clear();
        self.encrypted_files.clear();
        if let Ok(entries) = fs::read_dir(&self.notes_dir) {
            let q = self.search_query.to_lowercase();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "md") {
                    let fname = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                    
                    if let Ok(content) = fs::read_to_string(&path) {
                        // Track encryption status
                        if Vault::is_encrypted(&content) {
                            self.encrypted_files.insert(path.clone());
                        }

                        let matches = if self.search_query.is_empty() {
                            true
                        } else if fname.contains(&q) {
                            true
                        } else {
                            content.to_lowercase().contains(&q)
                        };

                        if matches {
                            self.files.push(path);
                        }
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
        self.pending_delete = false;
    }

    pub fn load_file(&mut self) {
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
                    self.active_file_is_encrypted = Vault::is_encrypted(&raw);

                    match Vault::decrypt(&raw, &self.master_password) {
                        Ok(content) => {
                            self.content = content;
                            if self.vault_active && !self.active_file_is_encrypted {
                                self.status_message = "NOTE IS PLAIN. Save (:w) to encrypt with vault.".into();
                            } else {
                                self.status_message = format!("Loaded: {}", path.file_name().unwrap_or_default().to_string_lossy());
                            }
                        }
                        Err(e) => {
                            self.content = e;
                            self.status_message = "Decryption failed or file locked.".into();
                        }
                    }
                    self.cursor_index = 0; 
                    self.is_modified = false;
                    self.editor_scroll_y = 0;
                    self.editor_scroll_x = 0;
                    self.visual_anchor = None;
                    self.undo_stack.clear();
                    self.redo_stack.clear();
                    self.pending_delete = false;
                }
            }
        }
    }

    pub fn save_file(&mut self) {
        if let Some(path) = &self.active_file {
            let data = Vault::encrypt(&self.content, &self.master_password, self.encrypt_bits);
            if let Err(e) = fs::write(path, &data) {
                self.status_message = format!("Error saving: {}", e);
            } else {
                let lock_mark = if self.vault_active {
                    format!(" [{}] ", self.encrypt_bits.label())
                } else {
                    String::new()
                };
                self.status_message = format!("Saved{}", lock_mark);
                self.is_modified = false;
                self.pending_quit = false;
                self.active_file_is_encrypted = self.vault_active;
                if self.vault_active {
                    self.encrypted_files.insert(path.clone());
                } else {
                    self.encrypted_files.remove(path);
                }
            }
        }
    }

    pub fn apply_vault_password(&mut self) {
        let password = self.prompt_buffer.clone();
        self.prompt_buffer.clear();
        self.mode = AppMode::Normal;

        if password.is_empty() {
            self.vault_active = false;
            self.master_password.clear();
            self.status_message = "Vault disabled. New saves will be plaintext.".into();
            return;
        }

        self.master_password = password;
        self.vault_active = true;

        if let Some(path) = self.active_file.clone() {
            let raw = fs::read_to_string(&path).unwrap_or_default();
            self.active_file_is_encrypted = Vault::is_encrypted(&raw);

            match Vault::decrypt(&raw, &self.master_password) {
                Ok(content) => {
                    self.content = content;
                    self.cursor_index = self.cursor_index.min(self.content.len());
                    
                    if !self.active_file_is_encrypted {
                        self.status_message = "Vault active. Save now to secure this note!".into();
                        self.is_modified = true; // Trigger save reminder
                    } else {
                        self.status_message = "Vault unlocked (AES-256-GCM) - [LOCKED]".into();
                    }
                }
                Err(e) => {
                    self.status_message = e;
                    self.vault_active = false;
                    self.master_password.clear();
                }
            }
        }
    }

    pub fn delete_file(&mut self) {
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

    pub fn create_new_file(&mut self) {
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

    pub fn rename_file(&mut self) {
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

    pub fn next_file(&mut self) {
        if self.files.is_empty() { return; }
        let i = match self.file_list_state.selected() {
            Some(i) => if i >= self.files.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.file_list_state.select(Some(i));
        self.load_file();
    }

    pub fn previous_file(&mut self) {
        if self.files.is_empty() { return; }
        let i = match self.file_list_state.selected() {
            Some(i) => if i == 0 { self.files.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.file_list_state.select(Some(i));
        self.load_file();
    }
    
    pub fn toggle_task_checkbox(&mut self) {
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

    pub fn update_scroll_to_cursor(&mut self, view_height: u16, _view_width: u16) {
        let (_, row) = self.get_visual_cursor();
        if row < self.editor_scroll_y {
            self.editor_scroll_y = row;
        } else if row >= self.editor_scroll_y + view_height {
            self.editor_scroll_y = row.saturating_sub(view_height).saturating_add(1);
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor_index, c);
        self.cursor_index += c.len_utf8();
        self.is_modified = true;
    }

    pub fn insert_string(&mut self, s: &str) {
        self.save_undo_state();
        self.content.insert_str(self.cursor_index, s);
        self.cursor_index += s.len();
        self.save_file();
    }

    pub fn delete_char(&mut self) {
        if self.cursor_index > 0 {
            let mut index = self.cursor_index - 1;
            while !self.content.is_char_boundary(index) { index -= 1; }
            self.content.remove(index);
            self.cursor_index = index;
            self.is_modified = true;
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_index > 0 {
            let mut index = self.cursor_index - 1;
            while !self.content.is_char_boundary(index) { index -= 1; }
            self.cursor_index = index;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_index < self.content.len() {
            let mut index = self.cursor_index + 1;
            while !self.content.is_char_boundary(index) { index += 1; }
            self.cursor_index = index;
        }
    }
    
    pub fn move_cursor_up(&mut self) {
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

    pub fn move_cursor_down(&mut self) {
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

    pub fn move_word_forward(&mut self) {
        let remaining = &self.content[self.cursor_index..];
        let mut iter = remaining.char_indices();
        
        // Consume current non-alphanumeric (if any)
        if let Some((_, c)) = iter.next() {
            if c.is_alphanumeric() {
                // We are in a word, skip to end of it
                for (idx, char) in iter.by_ref() {
                    if !char.is_alphanumeric() {
                        self.cursor_index += idx;
                        break;
                    }
                }
            }
        }
        
        // Skip whitespace/delimiters to next word
        let remaining = &self.content[self.cursor_index..];
        for (idx, char) in remaining.char_indices() {
            if char.is_alphanumeric() {
                self.cursor_index += idx;
                return;
            }
        }
        self.cursor_index = self.content.len();
    }

    pub fn move_word_backward(&mut self) {
        if self.cursor_index == 0 { return; }
        let before = &self.content[..self.cursor_index];
        let mut iter = before.char_indices().rev();
        
        // Skip whitespace/delimiters backwards
        let mut current_idx = self.cursor_index;
        for (idx, char) in iter.by_ref() {
            if char.is_alphanumeric() {
                current_idx = idx;
                break;
            }
            current_idx = idx;
        }
        
        // Now find start of this word
        let before = &self.content[..current_idx + 1];
        let iter = before.char_indices().rev();
        for (idx, char) in iter {
            if !char.is_alphanumeric() {
                self.cursor_index = idx + char.len_utf8();
                return;
            }
            self.cursor_index = idx;
        }
        self.cursor_index = 0;
    }

    pub fn move_to_top(&mut self) {
        self.cursor_index = 0;
        self.editor_scroll_y = 0;
    }

    pub fn move_to_bottom(&mut self) {
        self.cursor_index = self.content.len();
    }

    pub fn get_visual_cursor(&self) -> (u16, u16) {
        let text_before_cursor = &self.content[..self.cursor_index];
        let lines: Vec<&str> = text_before_cursor.split('\n').collect();
        let row = (lines.len() as u16).saturating_sub(1);
        let col = lines.last().map(|l| l.chars().count()).unwrap_or(0) as u16;
        (col, row)
    }

    pub fn enter_visual_mode(&mut self) {
        if self.active_file.is_some() {
            self.mode = AppMode::Visual;
            self.visual_anchor = Some(self.cursor_index);
            self.status_message = "Visual Selection".into();
        }
    }

    pub fn perform_yank(&mut self) {
        if let Some(anchor) = self.visual_anchor {
            let start = anchor.min(self.cursor_index);
            let end = anchor.max(self.cursor_index);
            self.clipboard = self.content[start..end].to_string();
            self.mode = AppMode::Normal;
            self.visual_anchor = None;
            self.status_message = format!("Yanked {} bytes.", self.clipboard.len());
        }
    }

    pub fn perform_cut(&mut self) {
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

    pub fn save_undo_state(&mut self) {
        if let Some(last) = self.undo_stack.last() {
            if last == &self.content {
                return;
            }
        }
        self.undo_stack.push(self.content.clone());
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn perform_undo(&mut self) {
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

    pub fn perform_redo(&mut self) {
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
