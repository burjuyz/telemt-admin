# Обзор проекта

`telemt-admin` — Telegram-бот для администрирования доступа к `telemt` (MTProto proxy).

Текущая целевая структура:

- Telegram handlers и экраны отвечают за UX и навигацию;
- SQLite-слой разделяется на узкие подмодули репозиториев в `src/db/*.rs`;
- интеграция с `telemt` проходит через единый backend-слой с API-first стратегией и fallback на `telemt.toml`/`systemd`;
- тексты интерфейса не должны собираться внутри SQL-запросов.

Основные сценарии:

- регистрация пользователя по invite-токену;
- ручное или автоматическое одобрение;
- выдача proxy-ссылки и QR;
- управление пользователями;
- управление `systemd`-сервисом `telemt` и наблюдение за control API.

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
- при включённом control API операции CRUD над пользователями должны идти через API; legacy-запись в `telemt.toml` используется как fallback;
- `telemt` поддерживает hot reload конфига, поэтому legacy-путь всё ещё может использовать `HUP/restart`, но это больше не основной механизм синхронизации;
- invite-токены учитывают срок действия, `is_active` и `max_usage`;
- критичные переходы состояний регистрации и consume invite-токена должны оставаться атомарными и не затирать конкурентные изменения;
- wizard-state хранится в SQLite, переживает рестарт процесса и может иметь TTL через `security.wizard_state_ttl_seconds`;
- если администратор запрашивает `/link` без существующей учётной записи, доступ создаётся автоматически.

Документация решений:

- индекс ADR: `docs/adr/README.md`;
- архитектурные решения по backend-слою: `docs/adr/001-telemt-api-backend.md`;
- решения по безопасности и rollout: `docs/adr/002-telemt-api-security-and-rollout.md`;
- операционный runbook: `docs/runbook.md`.
- backlog дальнейшей реализации: `docs/backlog.md`.
