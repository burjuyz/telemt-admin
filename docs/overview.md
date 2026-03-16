# Обзор проекта

`telemt-admin` — Telegram-бот для администрирования доступа к `telemt` (MTProto proxy).

Основные сценарии:

- регистрация пользователя по invite-токену;
- ручное или автоматическое одобрение;
- выдача proxy-ссылки и QR;
- управление пользователями;
- управление `systemd`-сервисом `telemt`.

Технологический стек:

- Rust edition `2024`
- `teloxide`
- `SQLite` через `sqlx`
- `toml` и `toml_edit`
- `tracing`, `tracing-subscriber`

Целевой production-deploy для текущего MVP:

- Linux x86_64 (glibc) с `systemd`;
- bootstrap-установка через `scripts/install.sh`.

Ключевые инварианты:

- пользователь `telemt` маппится как `tg_<telegram_user_id>`;
- `telemt` поддерживает hot reload конфига, поэтому после записи `telemt.toml` явный `restart` обычно не требуется;
- invite-токены учитывают срок действия, `is_active` и `max_usage`;
- wizard-state хранится в SQLite, переживает рестарт процесса и может иметь TTL через `security.wizard_state_ttl_seconds`;
- если администратор запрашивает `/link` без существующей учётной записи, доступ создаётся автоматически.
