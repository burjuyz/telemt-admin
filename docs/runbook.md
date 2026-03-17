# Runbook: rollout и эксплуатация `telemt` control API

## Назначение

Этот документ описывает, как включать и проверять API-first интеграцию `telemt-admin` с `telemt`, а также как откатываться на legacy-путь при проблемах.

## Предусловия

На стороне `telemt`:

- включён `[server.api]`;
- задан безопасный `listen`, обычно `127.0.0.1:9091`;
- настроен `auth_header`;
- при необходимости настроен `whitelist`.

На стороне `telemt-admin`:

- задана секция `[telemt_api]`;
- `base_url` указывает на фактический bind control API;
- `auth_header` в точности совпадает с `server.api.auth_header`.

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
   - security posture.

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
2. оставить `telemt` работающим в прежнем режиме;
3. перезапустить `telemt-admin`;
4. убедиться, что approve, `/link` и delete снова работают через legacy-path.
