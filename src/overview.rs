use std::{io::Stdout, sync::mpsc::Sender};

use crate::{cwl::AwsReq, status_bar, time_select, Mode, SelectedView, Widget};
use crossterm::event::KeyCode;
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub(crate) fn draw(
    app: std::sync::MutexGuard<crate::App>,
    frame: &mut Frame<CrosstermBackend<Stdout>>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Length(9),
                Constraint::Min(1),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(frame.size());

    let first_chunk = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(20)].as_ref())
        .split(chunks[0]);
    let selected_log_groups_string = app.selected_log_groups.join(", ");
    let log_groups = Paragraph::new(selected_log_groups_string.as_str())
        .style(match app.focused {
            Widget::LogGroups => Style::default().fg(Color::Yellow),
            _ => Style::default(),
        })
        .block(Block::default().borders(Borders::ALL).title("log groups"));
    frame.render_widget(log_groups, first_chunk[0]);

    time_select::draw(&app, frame, first_chunk[1]);

    let log_groups = Paragraph::new(app.query.as_str())
        .style(match app.focused {
            Widget::Query => Style::default().fg(Color::Yellow),
            _ => Style::default(),
        })
        .block(Block::default().borders(Borders::ALL).title("query"));
    frame.render_widget(log_groups, chunks[1]);

    let messages: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let content = vec![Spans::from(Span::raw(format!("{}: {}", i, m)))];
            ListItem::new(content).style(
                if app.focused != Widget::LogGroupsResults || app.mode != Mode::Insert {
                    Style::default()
                } else if i == app.log_group_row {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default()
                },
            )
        })
        .collect();
    let messages = List::new(messages).block(
        Block::default()
            .style(match app.focused {
                Widget::LogGroupsResults => Style::default().fg(Color::Yellow),
                _ => Style::default(),
            })
            .borders(Borders::ALL)
            .title("results"),
    );
    frame.render_widget(messages, chunks[2]);
    status_bar::draw(app, frame, chunks[3]);
}

pub(crate) fn handle_input(
    mut app: std::sync::MutexGuard<crate::App>,
    key_code: KeyCode,
    cwl: &Sender<AwsReq>,
) {
    match key_code {
        KeyCode::Enter => match app.focused {
            Widget::LogGroups => {
                app.selected = SelectedView::LogGroups;
                app.focused = Widget::LogGroups;
                if app.log_groups.is_empty() {
                    cwl.send(AwsReq::ListLogGroups).unwrap();
                }
            }
            Widget::Query => app.break_inner = true,
            Widget::TimeSelector => app.time_selector.popup = true,
            _ => {}
        },
        KeyCode::Char('h') | KeyCode::Left => match app.focused {
            Widget::LogGroups => {
                app.focused = Widget::TimeSelector;
            }
            Widget::TimeSelector => {
                app.focused = Widget::LogGroups;
            }
            _ => {}
        },
        KeyCode::Char('j') | KeyCode::Down => match app.focused {
            Widget::LogGroups | Widget::TimeSelector => {
                app.focused = Widget::Query;
            }
            _ => {
                app.focused = Widget::LogGroups;
            }
        },
        KeyCode::Char('k') | KeyCode::Up => match app.focused {
            Widget::LogGroups => {
                app.focused = Widget::Query;
            }
            _ => {
                app.focused = Widget::LogGroups;
            }
        },
        KeyCode::Char('l') | KeyCode::Right => match app.focused {
            Widget::LogGroups => {
                app.focused = Widget::TimeSelector;
            }
            Widget::TimeSelector => {
                app.focused = Widget::LogGroups;
            }
            _ => {}
        },
        KeyCode::Char('r') => {
            cwl.send(AwsReq::RunQuery).unwrap();
        }
        _ => {}
    }
}
