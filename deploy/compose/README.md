# Пример Docker Compose для `telemt-admin`

Подготовка рабочей директории:

```bash
mkdir -p telemt-admin-docker/config telemt-admin-docker/data
cd telemt-admin-docker
curl -fsSLo docker-compose.yml https://raw.githubusercontent.com/fgbm/telemt-admin/main/deploy/compose/docker-compose.telemt-admin.example.yml
curl -fsSLo .env.example https://raw.githubusercontent.com/fgbm/telemt-admin/main/deploy/compose/.env.example
curl -fsSLo config/telemt-admin.toml https://raw.githubusercontent.com/fgbm/telemt-admin/main/deploy/docker/telemt-admin.docker.toml.example
cp .env.example .env
sudo chown 65534:65534 data
```

После этого:

1. Заполните `.env` секретами и, при необходимости, зафиксируйте версию образа в `TELEMT_ADMIN_IMAGE`. Если из контейнера недоступен `api.telegram.org`, задайте `TELEMT_ADMIN__BOT_USERNAME` (username без `@`).
2. Отредактируйте `config/telemt-admin.toml`. Внутри контейнера он будет смонтирован как `/etc/telemt-admin/telemt-admin.toml`.
3. Проверьте, что `telemt_api.base_url` доступен из сети контейнера `telemt-admin`.

Минимальный пример `config/telemt-admin.toml`:

```toml
# bot_token можно не указывать, если задать TELEMT_ADMIN__BOT_TOKEN или TELOXIDE_TOKEN в .env
admin_ids = [123456789]

[runtime]
mode = "external"
label = "docker"

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

Обновление:

```bash
docker compose pull
docker compose up -d
docker image prune -f
```

`telemt-admin` и `telemt` должны находиться в одной пользовательской сети Docker, если `telemt_api.base_url` указывает на имя контейнера. Для других схем используйте reachable host/IP, `host` network или `extra_hosts` по необходимости.
