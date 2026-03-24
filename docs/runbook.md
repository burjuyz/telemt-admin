# Runbook: rollout и эксплуатация `telemt` control API

## Назначение

Этот документ описывает, как включать и проверять API-first интеграцию `telemt-admin` с `telemt`, а также как откатываться на legacy-путь при проблемах.

## Docker / `runtime = external`

Если бот запущен в контейнере без доступа к host systemd:

- задайте в `telemt-admin.toml` секцию `[runtime]` с `mode = "external"` (или `none`);
- убедитесь, что `[telemt_api].base_url` указывает на реальный адрес control API (имя сервиса в Docker-сети, а не только `127.0.0.1`);
- для чисто API-сценария рекомендуется `allow_file_fallback = false`.

Кнопки start/stop/restart/reload на экране `/service` в этих режимах скрыты; диагностика идёт через control API.

Переменные `TELEMT_ADMIN__*` (whitelist) применяются после TOML и удобны для Compose; см. `docs/adr/004-config-sources-and-docker-defaults.md`.

## Предусловия

На стороне `telemt`:

- включён `[server.api]`;
- задан безопасный `listen`, обычно `127.0.0.1:9091`;
- настроен `auth_header`;
- при необходимости настроен `whitelist`.

На стороне `telemt-admin`:

- задана секция `[telemt_api]`;
- `base_url` указывает на фактический bind control API;
- `auth_header` в точности совпадает с `server.api.auth_header`;
- если используется фоновый мониторинг, настроена секция `[notifications]`.

## Рекомендуемая конфигурация

Пример `telemt.toml`:

```toml
[server.api]
enabled = true
listen = "127.0.0.1:9091"
whitelist = ["127.0.0.1/32", "::1/128"]
auth_header = "Bearer <generated-secret>"
```

Пример `telemt-admin.toml`:

```toml
[telemt_api]
enabled = true
base_url = "http://127.0.0.1:9091"
auth_header = "Bearer <generated-secret>"
timeout_ms = 5000
allow_file_fallback = true
prefer_api_links = true

[notifications]
enabled = true
health_check_interval_secs = 60
notify_on_health_change = true
notify_on_runtime_alerts = true
notify_on_new_request = true
```

## Порядок rollout

### Фаза 1. Подготовка

- задеплоить версию `telemt-admin` с `telemt_backend`;
- оставить fallback включённым;
- проверить, что legacy-путь всё ещё работает.

### Фаза 2. Включение API backend

- настроить `[server.api]` в `telemt`;
- выполнить restart `telemt`, если менялись параметры `server.api`;
- включить `[telemt_api].enabled = true` в `telemt-admin`;
- проверить или задать политику `[notifications]` в `telemt-admin`;
- перезапустить `telemt-admin`.

### Фаза 3. Подтверждение работоспособности

Проверить вручную:

1. создать пользователя через auto-approve токен;
2. вручную одобрить pending-заявку;
3. запросить `/link` для уже существующего пользователя;
4. удалить пользователя;
5. открыть `/service` и убедиться, что видны:
   - systemd state;
   - health control API;
   - runtime/system info;
   - security posture;
   - блок нагрузки (`uptime`, connections, bad connections, handshake timeouts);
   - live connections (`total`, `ME`, `Direct`, `active users`);
   - экран `Top пользователей`.
6. открыть карточку пользователя и убедиться, что:
   - подгружаются live runtime-данные;
   - кнопки изменения лимитов работают через control API.
7. при включённых `[notifications]` проверить, что новые manual-заявки и health/runtime alerts приходят администраторам.

## Ожидаемое поведение fallback

Если `allow_file_fallback = true`, при сбое control API допустимо следующее:

- approve/create выполняется через legacy-конфиг и `systemd`;
- `/link` собирается локально по `secret` из SQLite;
- delete идёт через старый путь редактирования `telemt.toml`.

Fallback не должен быть скрытым operationally:

- причина деградации должна отражаться в логах;
- sync-метаданные пользователя в SQLite должны помогать понять, что произошло.

## Значение sync-полей в SQLite

- `backend_mode` — какой backend последним успешно применял изменения;
- `last_sync_error` — последнее известное описание проблемы синхронизации;
- `last_seen_revision` — последняя revision, полученная от control API;
- `last_synced_at` — время последней фиксации sync-состояния.

## Фоновый мониторинг и уведомления

Секция `[notifications]` управляет фоновым polling `telemt` API в `telemt-admin`.

- `enabled = true` включает фоновую задачу мониторинга;
- `health_check_interval_secs` задаёт интервал опроса;
- `notify_on_health_change` включает уведомления о:
  - недоступности/восстановлении control API;
  - смене health status;
  - смене `accepting_new_connections`;
  - смене `me_runtime_ready`;
- `notify_on_runtime_alerts` включает уведомления о:
  - появлении/исчезновении `unhealthy upstream`;
  - проблемах и восстановлении `ME self-test` (`KDF`, `time skew`);
- `notify_on_new_request` включает уведомления администраторам о новых manual-заявках.

Operational note:

- если на staging/production уведомления слишком шумные, сначала уменьшайте scope через флаги `notify_on_*`, а не выключайте весь `[telemt_api]`;
- при `notifications.enabled = false` бот остаётся полностью рабочим, но перестаёт слать push-уведомления и health/runtime alerts.

## Когда нужен restart `telemt`

Restart обязателен, если менялись параметры `[server.api]`, включая:

- `enabled`;
- `listen`;
- `whitelist`;
- `auth_header`.

Обычные CRUD-операции над пользователями через control API не должны требовать restart.

## Известные ограничения

- `POST /v1/users/{username}/rotate-secret` в текущем `telemt` не использовать;
- `/metrics` не заменяет control API;
- наличие fallback не должно оправдывать публичную экспозицию control API без auth.

## Откат

Если rollout проходит неуспешно:

1. установить `[telemt_api].enabled = false` в `telemt-admin.toml`;
2. при необходимости отключить `[notifications].enabled`, чтобы убрать фоновые API health-check;
3. оставить `telemt` работающим в прежнем режиме;
4. перезапустить `telemt-admin`;
5. убедиться, что approve, `/link` и delete снова работают через legacy-path.
