# Пример Docker Compose для `telemt-admin`

Подготовка рабочей директории:

```bash
mkdir -p telemt-admin-docker/config telemt-admin-docker/data
cd telemt-admin-docker
curl -fsSLo docker-compose.yml https://raw.githubusercontent.com/fgbm/telemt-admin/master/deploy/compose/docker-compose.telemt-admin.example.yml
curl -fsSLo .env.example https://raw.githubusercontent.com/fgbm/telemt-admin/master/deploy/compose/.env.example
curl -fsSLo config/telemt-admin.toml https://raw.githubusercontent.com/fgbm/telemt-admin/master/deploy/docker/telemt-admin.docker.toml.example
cp .env.example .env
```

После этого:

1. Отредактируйте `config/telemt-admin.toml`: `bot_token`, `admin_ids`, при необходимости `telemt_api.auth_header` и `base_url`. При необходимости задайте в `.env` `TELEMT_ADMIN_VERSION` (по умолчанию в образ уходит тег `latest`). Если из контейнера недоступен `api.telegram.org`, задайте `TELEMT_ADMIN__BOT_USERNAME` (username без `@`).
2. Внутри контейнера конфиг монтируется как `/etc/telemt-admin/telemt-admin.toml`.
3. Режим `external` и метка `docker` для UI задаются в `docker-compose.yml` (`TELEMT_ADMIN__RUNTIME__MODE`, `TELEMT_ADMIN__RUNTIME__LABEL`), переопределять в TOML не требуется.
4. Уровень логов — переменная `RUST_LOG` в `.env` (например `info` или `debug` для отладки).

Минимальный пример `config/telemt-admin.toml`:

```toml
bot_token = "ВАШ_ТОКЕН_БОТА"
admin_ids = [123456789]

db_path = "/var/lib/telemt-admin/state.db"
telemt_config_path = "/etc/telemt.toml"

[telemt_api]
enabled = true
base_url = "http://telemt:9091"
auth_header = "Bearer ..."
timeout_ms = 5000
allow_file_fallback = false
prefer_api_links = true
```

Запуск:

```bash
docker compose pull
docker compose up -d
docker compose logs -f telemt-admin
```

Если контейнер постоянно перезапускается, смотрите логи командой выше. Частые причины: не задан `bot_token`, неверный тег в `TELEMT_ADMIN_VERSION`, недоступен Telegram API из сети контейнера.

Обновление:

```bash
docker compose pull
docker compose up -d
docker image prune -f
```

`telemt-admin` и `telemt` должны находиться в одной пользовательской сети Docker, если `telemt_api.base_url` указывает на имя контейнера. Для других схем используйте reachable host/IP, `host` network или `extra_hosts` по необходимости.
