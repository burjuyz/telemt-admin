# AGENTS.md

## Команды разработки

```bash
cargo test --locked
cargo check --locked
cargo clippy --all-targets -- -D warnings
```

Порядок: `clippy` → `check` → `test`. CI проверяет все три.

## Критичные ограничения

- **Блокирующий файловый I/O** (`telemt.toml`, self-update unpack/copy/rename) — **всегда** через `spawn_blocking`, никогда не в async-executor.
- **`auth_header`** в `[telemt_api]` должен **в точности** совпадать с `server.api.auth_header` в `telemt.toml`.
- **`bot_username`** (без `@`) нужен, если из контейнера недоступен `api.telegram.org` для `getMe` — иначе ссылки на токены и deep link не соберутся.
- **`allow_file_fallback`** — только если есть RW mount `telemt.toml` и/или systemd.

## Архитектура (кратко)

- `src/main.rs` — вход, конфиг, БД, Dispatcher
- `src/db/*.rs` — SQLite-репозитории (registration, invite_tokens, user_groups, admin, wizard_state)
- `src/telemt_backend.rs` — единый фасад: API-first + legacy fallback
- `src/bot/handlers/actions/*.rs` — orchestration (approve, users, tokens, service, broadcast)
- `src/bot/handlers/screens.rs` / `keyboards.rs` — presentation, **не** данные
- `src/monitor.rs` — фоновый polling и уведомления, не трогает UI
- `src/runtime/` — systemd / external / none runtime abstraction
- `src/telemt_cfg.rs` — legacy-адаптер, блокирующий I/O через offload

Слой DB возвращает данные/доменные состояния, НЕ готовые UI-строки. UI-строки — в `[bot_messages]` конфига и `src/bot/handlers/format.rs`.

## БД и миграции

- Схема обновляется автоматически при старте (`Db::migrate()`).
- Миграции мягкие: новые таблицы, колонки, индексы. Сложные — явно.
- `invite_token_id` сохраняется в registration и используется в карточках/уведомлениях.
- Wizard-state хранится в SQLite, переживает рестарт процесса.
- Переходы состояний регистрации и consume-токена — атомарные условные UPDATE.

## Конфигурация

TOML → `TELEMT_ADMIN__*` overlay (whitelist, приоритет выше файла). Секреты — через env, не в Dockerfile.

## Важные инварианты

- Пользователь маппится как `tg_<telegram_user_id>`.
- Control API — основной путь; `telemt.toml` + systemd — fallback.
- `/service` и monitoring показывают частичную деградацию API по секциям, не маскируют.
- Auto-import: если `tg_<id>` уже есть в telemt API, user flow создаёт локальную approved-запись без ручного импорта.
- **`/link`** для админа без учётки создаёт доступ автоматически.
- **`/start`** поддерживает deep links: invite-token, карточка пользователя, карточка токена, admin screens.

## Когда менять документацию

- Архитектурные границы → `.cursor/rules/*.mdc` + `docs/architecture.md`
- Интеграция с telemt / fallback / security → ADR в `docs/adr/`
- Эксплуатация control API → `docs/runbook.md`
- UI/UX и notifications → `README.md`, `docs/overview.md`, `docs/runbook.md`
