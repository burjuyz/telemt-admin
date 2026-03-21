# Архитектура

Основные модули:

- `src/main.rs` — инициализация конфига, БД, состояния бота и `Dispatcher`.
- `src/config.rs` — загрузка `telemt-admin.toml`, дефолты и валидация.
- `src/monitor.rs` — фоновый polling `telemt` control API и push-уведомления администраторам.
- `src/db.rs` — корневой модуль SQLite-слоя с общими типами и публичным API.
- `src/db/migrations.rs` — мягкие миграции схемы и bootstrap БД.
- `src/db/registration.rs` — заявки, пользователи и переходы `pending/approved/rejected/deleted`.
- `src/db/invite_tokens.rs` — invite-токены, consume/revoke и проверки активного токена.
- `src/db/admin.rs` — агрегаты админки и структурированные события активности.
- `src/db/wizard_state.rs` — хранение wizard-state и TTL cleanup.
- `src/telemt_cfg.rs` — legacy-адаптер для чтения и изменения `telemt.toml`.
- `src/telemt_backend.rs` — единый backend-слой для работы с `telemt`: control API first, file/systemd fallback, live user info, stats и runtime snapshots.
- `src/service.rs` — async-обертка над `systemctl` и `journalctl`.
- `src/link.rs` — генерация секрета и `tg://proxy`-ссылки.
- `src/bot/handlers.rs` — сборка схемы обработчиков.
- `src/bot/handlers/commands/mod.rs` — slash-команды как точки входа в основные разделы и сценарии бота.
- `src/bot/handlers/callbacks/mod.rs` — inline callbacks и wizard-навигация.
- `src/bot/handlers/menu.rs` — текстовый ввод для активного wizard-state.
- `src/bot/keyboards.rs` — inline-клавиатуры.

Связанные документы:

- `docs/adr/README.md` — индекс ADR и точка входа в принятые решения.
- `docs/adr/001-telemt-api-backend.md` — почему выбран `API-first + fallback`.
- `docs/adr/002-telemt-api-security-and-rollout.md` — security-границы и стратегия rollout.
- `docs/runbook.md` — практическая эксплуатация и проверка rollout.

Принципы:

- Telegram/UI-логика не должна утекать в `db`, `service`, `telemt_cfg` и HTTP-клиент `telemt`.
- Инфраструктурные модули не должны возвращать готовые русские UI-строки, если вместо этого можно вернуть структурированный результат.
- Доменные операции лучше переиспользовать из общих функций, а не дублировать в командах и callbacks.
- Новые UX-сценарии желательно строить как `slash -> wizard/inline`, а не как роутинг по тексту сообщений.
- `/start` остаётся универсальной точкой входа: обычный user flow, invite-token deep link и admin deep link на конкретный экран/сущность.
- Источником истины для wizard-state должна оставаться SQLite, чтобы сценарии корректно восстанавливались после рестарта и TTL применялся единообразно.
- Блокирующие системные команды не должны выполняться напрямую в async Telegram flow.
- Фоновый мониторинг не должен дублировать UI-логику: он получает только структурированные snapshots и решает, слать ли уведомление.

Границы слоёв:

- `src/db/*.rs` возвращают данные и доменные состояния, а не готовую разметку экранов.
- `src/bot/handlers/screens.rs` и `src/bot/keyboards.rs` отвечают за presentation.
- orchestration уровня “БД + telemt backend + Telegram-ответ” лучше держать в action/use-case функциях, а не размазывать по callback/router-коду.
- `telemt_backend` должен оставаться единой точкой выбора между control API и legacy file/systemd path.
- `monitor` использует только `BotState` и структурированные ответы `telemt_backend`; он не должен напрямую читать БД-схему `telemt` или строить UI-экраны.

Новые runtime-возможности:

- `telemt_backend` умеет получать live-данные пользователя из `GET /v1/users/{username}` и менять лимиты через `PATCH /v1/users/{username}`;
- service panel показывает не только systemd status, но и runtime snapshots, нагрузку и top users;
- user card совмещает локальные sync-метаданные SQLite и live-данные из control API;
- фоновые alert-ы управляются секцией `[notifications]` и используют polling `monitor.rs`.

База данных:

- схема обновляется автоматически при старте через `Db::migrate()`;
- текущий миграционный подход подходит для мягких изменений:
  - новые таблицы;
  - новые колонки;
  - новые индексы;
- сложные преобразования схемы лучше оформлять отдельно и явно.
- переходы состояний регистрации и consume invite-токена должны обновляться условными запросами, чтобы конкурентные действия не затирали друг друга.
