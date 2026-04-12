mod app;
mod ui;
mod theme;
mod vault;
mod config;

use std::io::{self, stdout};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::{App, AppMode};
use crate::ui::ui;
use crate::vault::EncryptBits;

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
                            (KeyCode::Char('n'), _) => {
                                app.mode = AppMode::PromptNewFile;
                                app.prompt_buffer.clear();
                            }
                            (KeyCode::Char('/'), _) => {
                                app.mode = AppMode::SearchFile;
                                app.prompt_buffer.clear();
                            }
                            (KeyCode::Char('?'), _) => app.mode = AppMode::ShowHelp,
                            (KeyCode::Char('i'), _) => {
                                if app.active_file.is_some() {
                                    app.mode = AppMode::Insert;
                                    app.status_message = "Entering Insert Mode".into();
                                }
                            }
                            (KeyCode::Char('v'), _) => app.enter_visual_mode(),
                            (KeyCode::Char('p'), _) => {
                                if !app.clipboard.is_empty() {
                                    let s = app.clipboard.clone();
                                    app.insert_string(&s);
                                }
                            }
                            (KeyCode::Char('d'), _) => {
                                if app.pending_delete {
                                    app.pending_delete = false;
                                    app.delete_file();
                                } else {
                                    let name = app.active_file.as_ref()
                                        .and_then(|p| p.file_name())
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "this note".into());
                                    app.pending_delete = true;
                                    app.status_message = format!("Delete '{}'? Press 'd' again to confirm, any other key to cancel.", name);
                                }
                            }
                            (KeyCode::Char('w'), _) => app.move_word_forward(),
                            (KeyCode::Char('b'), _) => app.move_word_backward(),
                            (KeyCode::Char('G'), _) => app.move_to_bottom(),
                            (KeyCode::Char('g'), _) => {
                                if app.pending_g {
                                    app.move_to_top();
                                    app.pending_g = false;
                                } else {
                                    app.pending_g = true;
                                }
                            }
                            (KeyCode::Char(':'), _) => {
                                app.mode = AppMode::Command;
                                app.command_buffer = ":".to_string();
                            }
                            _ => {
                                app.pending_g = false;
                                if app.pending_delete {
                                    app.pending_delete = false;
                                    app.status_message = "Delete cancelled.".into();
                                }
                            }
                        },
                        AppMode::Insert => match (key.code, key.modifiers) {
                            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                                app.save_file();
                                app.mode = AppMode::Normal;
                            }
                            (KeyCode::Esc, _) => {
                                app.save_file();
                                app.mode = AppMode::Normal;
                            }
                            (KeyCode::Enter, _) => app.insert_char('\n'),
                            (KeyCode::Char(c), _) => app.insert_char(c),
                            (KeyCode::Backspace, _) => app.delete_char(),
                            (KeyCode::Left, _) => app.move_cursor_left(),
                            (KeyCode::Right, _) => app.move_cursor_right(),
                            (KeyCode::Up, _) => app.move_cursor_up(),
                            (KeyCode::Down, _) => app.move_cursor_down(),
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
                                    if app.is_modified && !app.pending_quit {
                                        app.pending_quit = true;
                                        app.status_message = "Unsaved changes! Press :q again to force quit.".into();
                                    } else {
                                        return Ok(());
                                    }
                                } else if app.command_buffer == ":wq" {
                                    app.save_file();
                                    return Ok(());
                                } else if app.command_buffer.starts_with(":theme ") {
                                    let theme_name = app.command_buffer.replace(":theme ", "");
                                    app.set_theme(&theme_name);
                                } else if app.command_buffer.starts_with(":vault") {
                                    let arg = app.command_buffer.trim_start_matches(":vault").trim();
                                    app.encrypt_bits = match arg {
                                        "64"  => EncryptBits::B64,
                                        "128" => EncryptBits::B128,
                                        "192" => EncryptBits::B192,
                                        _     => EncryptBits::B256,
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
                        AppMode::Visual => match (key.code, key.modifiers) {
                            (KeyCode::Esc, _) => {
                                app.mode = AppMode::Normal;
                                app.visual_anchor = None;
                            }
                            (KeyCode::Char('y'), _) => app.perform_yank(),
                            (KeyCode::Char('d'), _) => app.perform_cut(),
                            (KeyCode::Left, _) | (KeyCode::Char('h'), _) => app.move_cursor_left(),
                            (KeyCode::Right, _) | (KeyCode::Char('l'), _) => app.move_cursor_right(),
                            (KeyCode::Up, _) | (KeyCode::Char('k'), _) => app.move_cursor_up(),
                            (KeyCode::Down, _) | (KeyCode::Char('j'), _) => app.move_cursor_down(),
                            _ => {}
                        },
                    }
                }
            }
        }
    }
}
