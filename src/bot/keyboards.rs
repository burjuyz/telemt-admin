//! Inline-клавиатуры для экранов и wizard-сценариев.

use crate::bot::handlers::callback_data::{CallbackAction, ServiceAction};
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

fn page_nav_row(
    page: i64,
    total_pages: i64,
    previous: CallbackAction,
    current: CallbackAction,
    next: CallbackAction,
) -> Vec<InlineKeyboardButton> {
    vec![
        InlineKeyboardButton::callback("⬅️", previous.encode()),
        InlineKeyboardButton::callback(
            format!("📄 {}/{}", page, total_pages.max(1)),
            current.encode(),
        ),
        InlineKeyboardButton::callback("➡️", next.encode()),
    ]
}

fn refresh_home_row(refresh: CallbackAction) -> Vec<InlineKeyboardButton> {
    vec![
        InlineKeyboardButton::callback("🔄 Обновить", refresh.encode()),
        InlineKeyboardButton::callback("🏠 Главная", CallbackAction::ShowAdminHome.encode()),
    ]
}

fn refresh_lookup_row(refresh: CallbackAction, lookup: CallbackAction) -> Vec<InlineKeyboardButton> {
    vec![
        InlineKeyboardButton::callback("🔄 Обновить", refresh.encode()),
        InlineKeyboardButton::callback("🔎 Найти", lookup.encode()),
    ]
}

pub fn approve_reject_buttons(request_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::default().append_row(vec![
        InlineKeyboardButton::callback(
            "✅ Одобрить",
            CallbackAction::ApproveRequest {
                request_id,
                page: 1,
            }
            .encode(),
        ),
        InlineKeyboardButton::callback(
            "❌ Отклонить",
            CallbackAction::RejectRequest {
                request_id,
                page: 1,
            }
            .encode(),
        ),
    ])
}

pub fn user_home_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                "🔑 Ввести invite-токен",
                CallbackAction::PromptInviteToken.encode(),
            ),
            InlineKeyboardButton::callback(
                "❓ Инструкция",
                CallbackAction::ShowUsageGuide.encode(),
            ),
        ],
        vec![InlineKeyboardButton::callback(
            "🔗 Получить ссылку",
            CallbackAction::ShowUserLink.encode(),
        )],
        vec![InlineKeyboardButton::callback(
            "🔄 Обновить статус",
            CallbackAction::ShowUserHome.encode(),
        )],
    ])
}

pub fn guide_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::default().append_row(vec![InlineKeyboardButton::callback(
        "⬅️ Назад",
        CallbackAction::ShowUserHome.encode(),
    )])
}

pub fn admin_home_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                "📥 Заявки",
                CallbackAction::ShowPendingRequests.encode(),
            ),
            InlineKeyboardButton::callback(
                "👥 Пользователи",
                CallbackAction::ShowUsersPage { page: 1 }.encode(),
            ),
        ],
        vec![
            InlineKeyboardButton::callback("🎟 Токены", CallbackAction::ShowTokenMenu.encode()),
            InlineKeyboardButton::callback("⚙️ Сервис", CallbackAction::ShowServicePanel.encode()),
        ],
        vec![InlineKeyboardButton::callback(
            "📊 Статистика",
            CallbackAction::ShowStats.encode(),
        )],
    ])
}

pub fn pending_requests_keyboard(
    requests: &[(i64, String)],
    page: i64,
    total_pages: i64,
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = requests
        .iter()
        .map(|(request_id, title)| {
            vec![InlineKeyboardButton::callback(
                title.clone(),
                CallbackAction::OpenPendingRequest {
                    request_id: *request_id,
                    page,
                }
                .encode(),
            )]
        })
        .collect();

    let prev_page = if page > 1 { page - 1 } else { 1 };
    let next_page = if page < total_pages {
        page + 1
    } else {
        total_pages
    };

    rows.push(page_nav_row(
        page,
        total_pages,
        CallbackAction::ShowPendingRequestsPage { page: prev_page },
        CallbackAction::ShowPendingRequestsPage { page },
        CallbackAction::ShowPendingRequestsPage { page: next_page },
    ));
    rows.push(refresh_home_row(CallbackAction::ShowPendingRequestsPage {
        page,
    }));

    InlineKeyboardMarkup::new(rows)
}

pub fn pending_request_card_keyboard(request_id: i64, page: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                "✅ Одобрить",
                CallbackAction::ApproveRequest { request_id, page }.encode(),
            ),
            InlineKeyboardButton::callback(
                "❌ Отклонить",
                CallbackAction::RejectRequest { request_id, page }.encode(),
            ),
        ],
        vec![InlineKeyboardButton::callback(
            "⬅️ Назад к заявкам",
            CallbackAction::ShowPendingRequestsPage { page }.encode(),
        )],
    ])
}

pub fn pending_result_keyboard(page: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "⬅️ К заявкам",
            CallbackAction::ShowPendingRequestsPage { page }.encode(),
        )],
        vec![InlineKeyboardButton::callback(
            "🏠 Главная",
            CallbackAction::ShowAdminHome.encode(),
        )],
    ])
}

pub fn users_page_keyboard(
    users: &[(i64, String)],
    page: i64,
    total_pages: i64,
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for (tg_user_id, title) in users {
        rows.push(vec![InlineKeyboardButton::callback(
            format!("👤 {}", title),
            CallbackAction::OpenUserCard {
                tg_user_id: *tg_user_id,
                page,
            }
            .encode(),
        )]);
    }

    let prev_page = if page > 1 { page - 1 } else { 1 };
    let next_page = if page < total_pages {
        page + 1
    } else {
        total_pages
    };

    rows.push(page_nav_row(
        page,
        total_pages,
        CallbackAction::ShowUsersPage { page: prev_page },
        CallbackAction::ShowUsersPage { page },
        CallbackAction::ShowUsersPage { page: next_page },
    ));
    rows.push(refresh_lookup_row(
        CallbackAction::ShowUsersPage { page },
        CallbackAction::PromptUserLookup { page },
    ));
    rows.push(vec![
        InlineKeyboardButton::callback("➕ Создать", CallbackAction::PromptCreateUser.encode()),
        InlineKeyboardButton::callback("⛔ Удалить", CallbackAction::PromptDeleteUser.encode()),
    ]);
    rows.push(vec![InlineKeyboardButton::callback(
        "🏠 Главная",
        CallbackAction::ShowAdminHome.encode(),
    )]);

    InlineKeyboardMarkup::new(rows)
}

pub fn user_card_keyboard(tg_user_id: i64, page: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::default()
        .append_row(vec![InlineKeyboardButton::callback(
            "🔗 Данные + QR",
            CallbackAction::ViewUserQr { tg_user_id }.encode(),
        )])
        .append_row(vec![InlineKeyboardButton::callback(
            "⛔ Удалить пользователя",
            CallbackAction::ConfirmUserBan { tg_user_id, page }.encode(),
        )])
        .append_row(vec![InlineKeyboardButton::callback(
            "⬅️ Назад к списку",
            CallbackAction::ShowUsersPage { page }.encode(),
        )])
}

pub fn service_control_buttons() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                "🔄 Обновить",
                CallbackAction::ShowServicePanel.encode(),
            ),
            InlineKeyboardButton::callback(
                "📖 Reload",
                CallbackAction::ExecuteServiceAction {
                    action: ServiceAction::Reload,
                }
                .encode(),
            ),
        ],
        vec![
            InlineKeyboardButton::callback(
                "🔄 Перезапустить",
                CallbackAction::ConfirmServiceAction {
                    action: ServiceAction::Restart,
                }
                .encode(),
            ),
            InlineKeyboardButton::callback(
                "⏹ Остановить",
                CallbackAction::ConfirmServiceAction {
                    action: ServiceAction::Stop,
                }
                .encode(),
            ),
        ],
        vec![
            InlineKeyboardButton::callback(
                "▶️ Запустить",
                CallbackAction::ExecuteServiceAction {
                    action: ServiceAction::Start,
                }
                .encode(),
            ),
            InlineKeyboardButton::callback("🏠 Главная", CallbackAction::ShowAdminHome.encode()),
        ],
    ])
}

pub fn token_menu_keyboard(auto_approve_enabled: bool) -> InlineKeyboardMarkup {
    let mut rows = vec![
        vec![InlineKeyboardButton::callback(
            "📋 Список токенов",
            CallbackAction::ShowTokenListPage { page: 1 }.encode(),
        )],
        vec![InlineKeyboardButton::callback(
            "🎫 Создать ручной токен",
            CallbackAction::PromptTokenCreate {
                auto_approve: false,
            }
            .encode(),
        )],
    ];

    if auto_approve_enabled {
        rows.insert(
            1,
            vec![InlineKeyboardButton::callback(
                "🚀 Создать авто-токен",
                CallbackAction::PromptTokenCreate { auto_approve: true }.encode(),
            )],
        );
    }

    rows.push(vec![InlineKeyboardButton::callback(
        "🏠 Главная",
        CallbackAction::ShowAdminHome.encode(),
    )]);
    InlineKeyboardMarkup::new(rows)
}

pub fn token_list_keyboard(
    tokens: &[(i64, String)],
    page: i64,
    total_pages: i64,
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for (token_id, title) in tokens {
        rows.push(vec![InlineKeyboardButton::callback(
            format!("🎟 {}", title),
            CallbackAction::OpenTokenCard {
                token_id: *token_id,
                page,
            }
            .encode(),
        )]);
    }

    let prev_page = if page > 1 { page - 1 } else { 1 };
    let next_page = if page < total_pages {
        page + 1
    } else {
        total_pages
    };

    rows.push(page_nav_row(
        page,
        total_pages,
        CallbackAction::ShowTokenListPage { page: prev_page },
        CallbackAction::ShowTokenListPage { page },
        CallbackAction::ShowTokenListPage { page: next_page },
    ));
    rows.push(refresh_lookup_row(
        CallbackAction::ShowTokenListPage { page },
        CallbackAction::PromptTokenLookup { page },
    ));
    rows.push(vec![
        InlineKeyboardButton::callback("⬅️ Назад", CallbackAction::ShowTokenMenu.encode()),
        InlineKeyboardButton::callback("🏠 Главная", CallbackAction::ShowAdminHome.encode()),
    ]);

    InlineKeyboardMarkup::new(rows)
}

pub fn token_card_keyboard(token_id: i64, page: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "🗑 Отозвать токен",
            CallbackAction::ConfirmTokenRevoke { token_id, page }.encode(),
        )],
        vec![InlineKeyboardButton::callback(
            "⬅️ Назад к списку",
            CallbackAction::ShowTokenListPage { page }.encode(),
        )],
    ])
}

pub fn confirm_token_revoke_keyboard(token_id: i64, page: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "✅ Подтвердить",
            CallbackAction::ExecuteTokenRevoke { token_id, page }.encode(),
        ),
        InlineKeyboardButton::callback(
            "⬅️ Назад",
            CallbackAction::OpenTokenCard { token_id, page }.encode(),
        ),
    ]])
}

pub fn confirm_service_action_keyboard(action: ServiceAction) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "✅ Подтвердить",
            CallbackAction::ExecuteServiceAction { action }.encode(),
        ),
        InlineKeyboardButton::callback("⬅️ Назад", CallbackAction::ShowServicePanel.encode()),
    ]])
}

pub fn stats_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("🔄 Обновить", CallbackAction::ShowStats.encode()),
        InlineKeyboardButton::callback("🏠 Главная", CallbackAction::ShowAdminHome.encode()),
    ]])
}

pub fn cancel_keyboard(back_action: CallbackAction) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("⬅️ Назад", back_action.encode()),
        InlineKeyboardButton::callback("✖️ Отмена", CallbackAction::CancelWizard.encode()),
    ]])
}

pub fn confirm_delete_keyboard(tg_user_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "✅ Да, удалить",
            CallbackAction::ExecuteDeleteUser { tg_user_id }.encode(),
        ),
        InlineKeyboardButton::callback("✖️ Отмена", CallbackAction::ShowAdminHome.encode()),
    ]])
}

pub fn confirm_user_ban_keyboard(tg_user_id: i64, page: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "✅ Подтвердить",
            CallbackAction::ExecuteUserBan { tg_user_id, page }.encode(),
        ),
        InlineKeyboardButton::callback(
            "⬅️ Назад",
            CallbackAction::OpenUserCard { tg_user_id, page }.encode(),
        ),
    ]])
}
