use std::fs;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
    Frame,
};
use crate::app::{App, AppMode};

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

pub fn generate_highlighted_lines<'a>(app: &'a App) -> Vec<Line<'a>> {
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
                    Style::default().bg(Color::White).fg(Color::Black)
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

pub fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    let vault_label = if app.vault_active {
        format!(" [{}] ", app.encrypt_bits.label())
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

    let (left_pane_width, right_pane_width) = if app.is_zen_mode {
        (0, 100)
    } else {
        (25, 75)
    };

    let main_panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(left_pane_width), Constraint::Percentage(right_pane_width)])
        .split(chunks[1]);

    if !app.is_zen_mode {
        let note_count = app.files.len();
        let active_path = app.active_file.clone();

        let items: Vec<ListItem> = app.files.iter().map(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            let size_str = fs::metadata(p).map(|m| {
                let b = m.len();
                if b >= 1024 { format!("{} KB", b / 1024) } else { format!("{} B", b) }
            }).unwrap_or_default();
            let lock = if app.vault_active && active_path.as_ref() == Some(p) { "[*] " } else { "" };
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
        Some(p) => p.file_name().unwrap_or_default().to_string_lossy().to_string(),
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
        [ Movement ]\n\
        w, b         : Next/Previous word\n\
        gg, G        : Jump to Top/Bottom\n\
        h, j, k, l   : Manual movements\n\
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
            
        let paragraph = Paragraph::new(p_text.as_str()).block(block).style(Style::default().fg(Color::Reset));
        
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
        let v_margin = (f.area().height.saturating_sub(3)) / 2;
        let area_v = Layout::default().direction(Direction::Vertical).constraints([
            Constraint::Length(v_margin), Constraint::Length(5), Constraint::Min(0)]).split(f.area())[1];
        let h_margin = (f.area().width.saturating_sub(44)) / 2;
        let area = Layout::default().direction(Direction::Horizontal).constraints([
            Constraint::Length(h_margin), Constraint::Length(44), Constraint::Min(0)]).split(area_v)[1];

        let masked = "*".repeat(app.prompt_buffer.len());
        let vault_status = if app.vault_active { " [ACTIVE] " } else { " [OFF] " };
        let p_text = format!("Password{}: {}", vault_status, masked);
        let hint = "\n(Enter to confirm, Esc to cancel, empty=disable)";
        let full_text = format!("{}{}", p_text, hint);

        let block = Block::default()
            .title(" Vault Encryption ")
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .style(Style::default().bg(Color::Reset).fg(Color::Magenta));

        let paragraph = Paragraph::new(full_text.as_str())
            .block(block)
            .style(Style::default().fg(Color::White));

        f.render_widget(Clear, area);
        f.render_widget(paragraph, area);
        let prefix_len = format!("Password{}:  ", vault_status).chars().count() as u16;
        f.set_cursor_position((area.x + 1 + prefix_len + masked.len() as u16, area.y + 1));
    }
}
