use iced::widget::{
    button, checkbox, column, container, mouse_area, pick_list, row, scrollable, text,
    text_input, Space,
};
use iced::{Color, Element, Length, Task};
use wuddle_engine::auto_login::{
    AccountDetails, AccountId, AccountRef, AutoLoginService, CredentialInput, SecretText,
};

use crate::components::helpers::close_button;
use crate::theme::{self, ThemeColors};
use crate::{App, Dialog, LogLevel, Message, ToastKind};

#[derive(Debug, Clone)]
pub struct AccountChoice {
    pub id: Option<AccountId>,
    label: String,
}

impl PartialEq for AccountChoice {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for AccountChoice {}

impl std::fmt::Display for AccountChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Debug, Clone, Default)]
pub struct EditorState {
    pub account_id: Option<AccountId>,
    pub label: String,
    pub login: SecretText,
    pub password: SecretText,
    pub realmlist: SecretText,
    pub realm_name: SecretText,
    pub warning_acknowledged: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UiState {
    pub editor: EditorState,
    pub loading: bool,
    pub saving: bool,
}

pub fn update(app: &mut App, message: Message) -> Option<Task<Message>> {
    match message {
        Message::OpenAutoLoginAccounts => {
            app.auto_login_ui = UiState::default();
            app.dialog = Some(Dialog::AutoLoginAccounts);
            Some(Task::none())
        }
        Message::SetAutoLoginAccountPickerTooltipVisible(visible) => {
            app.auto_login_account_picker_tooltip_visible = visible;
            Some(Task::none())
        }
        Message::DismissAutoLoginAccountPickerTooltip => {
            app.auto_login_account_picker_tooltip_visible = false;
            Some(Task::none())
        }
        Message::AddAutoLoginAccount => {
            app.auto_login_ui.editor = EditorState {
                warning_acknowledged: app.auto_login_warning_acknowledged,
                ..EditorState::default()
            };
            app.dialog = Some(Dialog::AutoLoginEditor);
            Some(Task::none())
        }
        Message::EditAutoLoginAccount(account_id) => {
            let profile_id = app.active_profile_id.clone();
            app.auto_login_ui.loading = true;
            app.dialog = Some(Dialog::AutoLoginEditor);
            Some(Task::perform(
                load_account(profile_id.clone(), account_id.clone()),
                move |result| Message::AutoLoginAccountLoaded {
                    profile_id: profile_id.clone(),
                    account_id: account_id.clone(),
                    result,
                },
            ))
        }
        Message::AutoLoginAccountLoaded {
            profile_id,
            account_id,
            result,
        } => {
            app.auto_login_ui.loading = false;
            if profile_id != app.active_profile_id {
                return Some(Task::none());
            }
            match result {
                Ok(details) => {
                    let label = app
                        .active_profile()
                        .and_then(|profile| {
                            profile
                                .auto_login_accounts
                                .iter()
                                .find(|account| account.id == account_id)
                        })
                        .map(|account| account.label.clone())
                        .unwrap_or_default();
                    app.auto_login_ui.editor = EditorState {
                        account_id: Some(account_id),
                        label,
                        login: SecretText::new(details.login),
                        password: SecretText::default(),
                        realmlist: SecretText::new(details.realmlist),
                        realm_name: SecretText::new(details.realm_name),
                        warning_acknowledged: app.auto_login_warning_acknowledged,
                    };
                }
                Err(error) => {
                    app.dialog = Some(Dialog::AutoLoginAccounts);
                    app.log(
                        LogLevel::Error,
                        &format!("Could not load auto-login account: {error}"),
                    );
                    app.show_toast(
                        format!("Could not load auto-login account: {error}"),
                        ToastKind::Error,
                    );
                }
            }
            Some(Task::none())
        }
        Message::SetAutoLoginLabel(value) => {
            app.auto_login_ui.editor.label = value;
            Some(Task::none())
        }
        Message::SetAutoLoginLogin(value) => {
            app.auto_login_ui.editor.login = value;
            Some(Task::none())
        }
        Message::SetAutoLoginPassword(value) => {
            app.auto_login_ui.editor.password = value;
            Some(Task::none())
        }
        Message::SetAutoLoginRealmlist(value) => {
            app.auto_login_ui.editor.realmlist = value;
            Some(Task::none())
        }
        Message::SetAutoLoginRealmName(value) => {
            app.auto_login_ui.editor.realm_name = value;
            Some(Task::none())
        }
        Message::ToggleAutoLoginWarningAcknowledged(value) => {
            app.auto_login_ui.editor.warning_acknowledged = value;
            Some(Task::none())
        }
        Message::SaveAutoLoginAccount => save_editor(app),
        Message::SaveAutoLoginAccountResult {
            profile_id,
            account,
            is_new,
            result,
        } => {
            app.auto_login_ui.saving = false;
            if profile_id != app.active_profile_id {
                return Some(Task::none());
            }
            match result {
                Ok(()) => {
                    let previous_profile = app
                        .profiles
                        .iter()
                        .find(|profile| profile.id == profile_id)
                        .cloned();
                    let previous_acknowledgement = app.auto_login_warning_acknowledged;
                    app.auto_login_warning_acknowledged = true;
                    if let Some(profile) = app
                        .profiles
                        .iter_mut()
                        .find(|profile| profile.id == profile_id)
                    {
                        if let Some(existing) = profile
                            .auto_login_accounts
                            .iter_mut()
                            .find(|existing| existing.id == account.id)
                        {
                            existing.label = account.label.clone();
                        } else {
                            profile.auto_login_accounts.push(account.clone());
                        }
                        if is_new {
                            profile.selected_auto_login_account_id = Some(account.id.clone());
                        }
                    }
                    if let Err(error) = app.try_save_settings() {
                        if let Some(previous_profile) = previous_profile {
                            if let Some(profile) = app
                                .profiles
                                .iter_mut()
                                .find(|profile| profile.id == profile_id)
                            {
                                *profile = previous_profile;
                            }
                        }
                        app.auto_login_warning_acknowledged = previous_acknowledgement;
                        app.log(
                            LogLevel::Error,
                            &format!("Could not save auto-login account metadata: {error}"),
                        );
                        app.show_toast(
                            format!("Could not save auto-login account metadata: {error}"),
                            ToastKind::Error,
                        );
                        if is_new {
                            return Some(Task::perform(
                                delete_account(profile_id.clone(), account.id.clone()),
                                Message::RollbackAutoLoginAccountResult,
                            ));
                        }
                        return Some(Task::none());
                    }
                    app.dialog = Some(Dialog::AutoLoginAccounts);
                    app.auto_login_ui.editor = EditorState::default();
                    app.log(
                        LogLevel::Info,
                        "Auto-login account saved in secure storage.",
                    );
                    app.show_toast("Auto-login account saved securely.", ToastKind::Success);
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Could not save auto-login account: {error}"),
                    );
                    app.show_toast(
                        format!("Could not save auto-login account: {error}"),
                        ToastKind::Error,
                    );
                }
            }
            Some(Task::none())
        }
        Message::RollbackAutoLoginAccountResult(result) => {
            if let Err(error) = result {
                app.log(
                    LogLevel::Error,
                    &format!(
                        "Could not roll back the secure credential after settings failed: {error}"
                    ),
                );
                app.show_toast(
                    format!("Settings failed and the secure credential could not be rolled back: {error}"),
                    ToastKind::Error,
                );
            }
            Some(Task::none())
        }
        Message::SelectAutoLoginAccount(account_id) => {
            app.auto_login_account_picker_tooltip_visible = false;
            if let Some(profile) = app
                .profiles
                .iter_mut()
                .find(|profile| profile.id == app.active_profile_id)
            {
                let valid = account_id.as_ref().map_or(true, |selected| {
                    profile
                        .auto_login_accounts
                        .iter()
                        .any(|account| &account.id == selected)
                });
                if valid {
                    let previous = profile.selected_auto_login_account_id.clone();
                    profile.selected_auto_login_account_id = account_id;
                    if let Err(error) = app.try_save_settings() {
                        if let Some(profile) = app
                            .profiles
                            .iter_mut()
                            .find(|profile| profile.id == app.active_profile_id)
                        {
                            profile.selected_auto_login_account_id = previous;
                        }
                        app.show_toast(
                            format!("Could not save account selection: {error}"),
                            ToastKind::Error,
                        );
                    }
                }
            }
            Some(Task::none())
        }
        Message::DeleteAutoLoginAccount(account_id) => {
            if let Some(account) = app.active_profile().and_then(|profile| {
                profile
                    .auto_login_accounts
                    .iter()
                    .find(|account| account.id == account_id)
            }) {
                app.dialog = Some(Dialog::DeleteAutoLoginAccount {
                    account_id,
                    label: account.label.clone(),
                });
            }
            Some(Task::none())
        }
        Message::ConfirmDeleteAutoLoginAccount => {
            let Some(Dialog::DeleteAutoLoginAccount { account_id, .. }) = app.dialog.clone() else {
                return Some(Task::none());
            };
            let profile_id = app.active_profile_id.clone();
            Some(Task::perform(
                delete_account(profile_id.clone(), account_id.clone()),
                move |result| Message::DeleteAutoLoginAccountResult {
                    profile_id: profile_id.clone(),
                    account_id: account_id.clone(),
                    result,
                },
            ))
        }
        Message::DeleteAutoLoginAccountResult {
            profile_id,
            account_id,
            result,
        } => {
            match result {
                Ok(()) => {
                    if let Some(profile) = app
                        .profiles
                        .iter_mut()
                        .find(|profile| profile.id == profile_id)
                    {
                        profile
                            .auto_login_accounts
                            .retain(|account| account.id != account_id);
                        if profile.selected_auto_login_account_id.as_ref() == Some(&account_id) {
                            profile.selected_auto_login_account_id = None;
                        }
                    }
                    if let Err(error) = app.try_save_settings() {
                        app.log(LogLevel::Error, &format!("Credential was removed, but account metadata could not be saved: {error}"));
                        app.show_toast(
                            format!(
                                "Credential was removed, but settings could not be saved: {error}"
                            ),
                            ToastKind::Error,
                        );
                        app.dialog = Some(Dialog::AutoLoginAccounts);
                        return Some(Task::none());
                    }
                    app.dialog = Some(Dialog::AutoLoginAccounts);
                    app.log(
                        LogLevel::Info,
                        "Auto-login account removed from secure storage.",
                    );
                    app.show_toast("Auto-login account removed.", ToastKind::Success);
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Could not remove auto-login account: {error}"),
                    );
                    app.show_toast(
                        format!("Could not remove auto-login account: {error}"),
                        ToastKind::Error,
                    );
                }
            }
            Some(Task::none())
        }
        _ => None,
    }
}

fn save_editor(app: &mut App) -> Option<Task<Message>> {
    if !app.auto_login_warning_acknowledged && !app.auto_login_ui.editor.warning_acknowledged {
        app.show_toast(
            "Acknowledge the command-line exposure warning first.",
            ToastKind::Warn,
        );
        return Some(Task::none());
    }
    let profile_id = app.active_profile_id.clone();
    let editor = app.auto_login_ui.editor.clone();
    let accounts = app
        .active_profile()
        .map(|profile| profile.auto_login_accounts.clone())
        .unwrap_or_default();
    let label = match AccountRef::validate_unique_label(
        &editor.label,
        &accounts,
        editor.account_id.as_ref(),
    ) {
        Ok(label) => label,
        Err(error) => {
            app.show_toast(error.to_string(), ToastKind::Warn);
            return Some(Task::none());
        }
    };
    let is_new = editor.account_id.is_none();
    let account = AccountRef {
        id: editor.account_id.unwrap_or_default(),
        label,
    };
    let input = match CredentialInput::new(
        editor.login.expose().to_string(),
        if editor.password.is_empty() {
            None
        } else {
            Some(editor.password.expose().to_string())
        },
        editor.realmlist.expose().to_string(),
        editor.realm_name.expose().to_string(),
    ) {
        Ok(input) => input,
        Err(error) => {
            app.show_toast(error.to_string(), ToastKind::Warn);
            return Some(Task::none());
        }
    };
    app.auto_login_ui.saving = true;
    Some(Task::perform(
        save_account(profile_id.clone(), account.id.clone(), input),
        move |result| Message::SaveAutoLoginAccountResult {
            profile_id: profile_id.clone(),
            account: account.clone(),
            is_new,
            result,
        },
    ))
}

async fn load_account(profile_id: String, account_id: AccountId) -> Result<AccountDetails, String> {
    tokio::task::spawn_blocking(move || {
        AutoLoginService::system()
            .load_account_details(&profile_id, &account_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn save_account(
    profile_id: String,
    account_id: AccountId,
    input: CredentialInput,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        AutoLoginService::system()
            .save_account(&profile_id, &account_id, input)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn delete_account(profile_id: String, account_id: AccountId) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        AutoLoginService::system()
            .delete_account(&profile_id, &account_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn delete_profile_accounts(
    profile_id: String,
    accounts: Vec<AccountRef>,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let service = AutoLoginService::system();
        for account in accounts {
            service
                .delete_account(&profile_id, &account.id)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub fn account_picker<'a>(app: &'a App, colors: ThemeColors) -> Element<'a, Message> {
    let profile = app.active_profile().cloned().unwrap_or_default();
    if !profile.auto_login_enabled {
        return Space::new().width(0).into();
    }
    let mut choices = vec![AccountChoice {
        id: None,
        label: "Manual Login".to_string(),
    }];
    choices.extend(
        profile
            .auto_login_accounts
            .iter()
            .map(|account| AccountChoice {
                id: Some(account.id.clone()),
                label: account.label.clone(),
            }),
    );
    let selected = choices
        .iter()
        .find(|choice| choice.id == profile.selected_auto_login_account_id)
        .cloned()
        .or_else(|| choices.first().cloned());
    let c = colors;
    let selector_tooltip: Element<'a, Message> = if app.auto_login_account_picker_tooltip_visible {
        container(
            text("Select account for auto-login. Requires 'Awesome WotLK'.")
                .size(13)
                .color(c.text),
        )
        .padding([6, 10])
        .style(move |_theme| theme::tooltip_style(c))
        .into()
    } else {
        Space::new().width(0).height(0).into()
    };
    let account_selector = iced::widget::tooltip(
        mouse_area(
            pick_list(choices, selected, |choice| Message::SelectAutoLoginAccount(choice.id))
                .text_size(12)
                .width(150),
        )
        .on_enter(Message::SetAutoLoginAccountPickerTooltipVisible(true))
        .on_exit(Message::SetAutoLoginAccountPickerTooltipVisible(false)),
        selector_tooltip,
        iced::widget::tooltip::Position::Top,
    )
    .padding(0);
    let manage_accounts = iced::widget::tooltip(
        button(
            container(text("⚙").size(20))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .on_press(Message::OpenAutoLoginAccounts)
        .padding(0)
        .width(30)
        .height(30)
        .style(move |_theme, status| match status {
            button::Status::Hovered | button::Status::Pressed => button::Style {
                background: None,
                text_color: c.primary_text,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            _ => button::Style {
                background: None,
                text_color: Color { a: 0.62, ..c.text },
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
        }),
        container(text("Manage auto-login accounts").size(13).color(c.text))
            .padding([6, 10])
            .style(move |_theme| theme::tooltip_style(c)),
        iced::widget::tooltip::Position::Top,
    );
    row![
        account_selector,
        manage_accounts,
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

pub fn view_dialog<'a>(
    app: &'a App,
    dialog: &'a Dialog,
    colors: ThemeColors,
) -> Element<'a, Message> {
    match dialog {
        Dialog::AutoLoginAccounts => accounts_dialog(app, colors),
        Dialog::AutoLoginEditor => editor_dialog(app, colors),
        Dialog::DeleteAutoLoginAccount { label, .. } => delete_dialog(label, colors),
        _ => Space::new().into(),
    }
}

fn accounts_dialog<'a>(app: &'a App, colors: ThemeColors) -> Element<'a, Message> {
    let accounts = app
        .active_profile()
        .map(|profile| profile.auto_login_accounts.as_slice())
        .unwrap_or_default();
    let mut rows: Vec<Element<Message>> = Vec::new();
    for account in accounts {
        let edit_id = account.id.clone();
        let delete_id = account.id.clone();
        let c = colors;
        rows.push(
            container(
                row![
                    text(&account.label).size(14).color(colors.text),
                    Space::new().width(Length::Fill),
                    button(text("Edit").size(12))
                        .on_press(Message::EditAutoLoginAccount(edit_id))
                        .padding([5, 10])
                        .style(move |_theme, _| theme::tab_button_style(c)),
                    button(text("Remove").size(12))
                        .on_press(Message::DeleteAutoLoginAccount(delete_id))
                        .padding([5, 10])
                        .style(move |_theme, _| theme::btn_danger_style(c)),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            )
            .padding([8, 10])
            .style(move |_| theme::card_style(c))
            .into(),
        );
    }
    if rows.is_empty() {
        rows.push(
            text("No auto-login accounts saved for this instance.")
                .size(13)
                .color(colors.muted)
                .into(),
        );
    }
    let c = colors;
    column![
        row![
            text("Auto-Login Accounts").size(18).color(colors.title),
            Space::new().width(Length::Fill),
            close_button(colors),
        ]
        .align_y(iced::Alignment::Center),
        text("Credentials are stored in your system vault. They are passed to the game as command-line arguments and may be readable by same-user or administrator tools while the game runs.")
            .size(12)
            .color(colors.warn),
        text("This requires Awesome WotLK or another compatible client modification. Only use it where the server permits modified clients.")
            .size(12)
            .color(colors.muted),
        scrollable(column(rows).spacing(6)).height(Length::Shrink),
        row![
            Space::new().width(Length::Fill),
            button(text("Add Account").size(13))
                .on_press(Message::AddAutoLoginAccount)
                .padding([7, 14])
                .style(move |_theme, _| theme::tab_button_active_style(c)),
        ],
    ]
    .spacing(12)
    .into()
}

fn editor_dialog<'a>(app: &'a App, colors: ThemeColors) -> Element<'a, Message> {
    if app.auto_login_ui.loading {
        return column![
            row![
                text("Auto-Login Account").size(18).color(colors.title),
                Space::new().width(Length::Fill),
                close_button(colors),
            ],
            text("Loading from secure storage…")
                .size(13)
                .color(colors.muted),
        ]
        .spacing(12)
        .into();
    }
    let editor = &app.auto_login_ui.editor;
    let password_hint = if editor.account_id.is_some() {
        "Leave blank to keep the existing password"
    } else {
        "Required"
    };
    let warning: Element<Message> = if app.auto_login_warning_acknowledged {
        text("No terminal window is opened. The credential is still visible in the game process command line to sufficiently privileged local tools.")
            .size(11)
            .color(colors.warn)
            .into()
    } else {
        checkbox(editor.warning_acknowledged)
            .label("I understand credentials are exposed in the game process command line while it runs.")
            .on_toggle(Message::ToggleAutoLoginWarningAcknowledged)
            .into()
    };
    let save = button(
        text(if app.auto_login_ui.saving {
            "Saving…"
        } else {
            "Save"
        })
        .size(13),
    )
    .padding([7, 14]);
    let save = if app.auto_login_ui.saving {
        save
    } else {
        save.on_press(Message::SaveAutoLoginAccount)
    };
    let c = colors;
    column![
        row![
            text(if editor.account_id.is_some() {
                "Edit Auto-Login Account"
            } else {
                "Add Auto-Login Account"
            })
            .size(18)
            .color(colors.title),
            Space::new().width(Length::Fill),
            close_button(colors),
        ]
        .align_y(iced::Alignment::Center),
        text("Account label").size(13).color(colors.text),
        text_input("Main account", &editor.label)
            .on_input(Message::SetAutoLoginLabel)
            .padding([8, 12]),
        text("Login").size(13).color(colors.text),
        text_input("Account name or email", editor.login.expose())
            .on_input(|value| Message::SetAutoLoginLogin(SecretText::new(value)))
            .padding([8, 12]),
        text("Password").size(13).color(colors.text),
        text_input(password_hint, editor.password.expose())
            .secure(true)
            .on_input(|value| Message::SetAutoLoginPassword(SecretText::new(value)))
            .padding([8, 12]),
        text("Realmlist (optional)").size(13).color(colors.text),
        text_input("logon.example.com", editor.realmlist.expose())
            .on_input(|value| Message::SetAutoLoginRealmlist(SecretText::new(value)))
            .padding([8, 12]),
        text("Realm name (optional)").size(13).color(colors.text),
        text_input("Realm Name", editor.realm_name.expose())
            .on_input(|value| Message::SetAutoLoginRealmName(SecretText::new(value)))
            .padding([8, 12]),
        warning,
        row![
            Space::new().width(Length::Fill),
            save.style(move |_theme, _| theme::tab_button_active_style(c))
        ],
    ]
    .spacing(7)
    .into()
}

fn delete_dialog<'a>(label: &'a str, colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;
    column![
        text("Remove Auto-Login Account")
            .size(18)
            .color(colors.title),
        text(format!(
            "Remove ‘{label}’ from secure storage? This cannot be undone."
        ))
        .size(13)
        .color(colors.text),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(13))
                .on_press(Message::OpenAutoLoginAccounts)
                .padding([7, 14])
                .style(move |_theme, _| theme::tab_button_style(c)),
            button(text("Remove").size(13))
                .on_press(Message::ConfirmDeleteAutoLoginAccount)
                .padding([7, 14])
                .style(move |_theme, _| theme::btn_danger_style(c)),
        ]
        .spacing(8),
    ]
    .spacing(14)
    .into()
}
