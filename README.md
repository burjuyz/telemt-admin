<p align="center">
  <img src="logo.png" alt="telemt-admin logo" width="800">
</p>

<h1 align="center">telemt-admin</h1>

**telemt-admin** — это Telegram-бот для автоматизации управления пользователями [telemt](https://github.com/telemt/telemt) (MTProto прокси-сервера). Проект создан для системных администраторов, которым нужно делегировать процесс выдачи доступа, избавившись от ручного редактирования конфигурационных файлов.

Поддерживает **систему пригласительных токенов** с двумя режимами работы:

- **Ручной (Manual):** пользователь подает заявку, админ подтверждает.
- **Автоматический (Auto-approve):** пользователь мгновенно получает доступ без участия админа.

## Содержание

- [Зачем это нужно?](#зачем-это-нужно)
- [Требования](#требования)
- [Быстрый старт (Linux)](#быстрый-старт-linux)
- [Ручная установка](#ручная-установка)
- [Установка как системного сервиса](#установка-как-системного-сервиса)
- [Как пользоваться](#как-пользоваться)
- [Конфигурация (telemt-admin.toml)](#конфигурация-telemt-admintoml)
- [Проверка после запуска](#проверка-после-запуска)
- [Обновление](#обновление)
- [Сборка из исходников](#сборка-из-исходников)
- [Архитектурные решения и runbook](#архитектурные-решения-и-runbook)
- [Troubleshooting](#troubleshooting)
- [CI/CD](#cicd)

## Зачем это нужно?

Традиционное управление пользователями в `telemt` требует прямого доступа к серверу и ручного изменения `/etc/telemt.toml`.
**telemt-admin** переносит этот процесс в Telegram:

- **Для пользователей:** простая регистрация через бота по пригласительному токену.
- **Для администраторов:** гибкое управление доступом (временные токены, лимиты использований), мгновенные уведомления и управление через кнопки.
- **Для сервера:** управление пользователями через control API `telemt` с fallback на legacy-конфиг и `systemd`.

Интерфейс бота построен по модели, близкой к `BotFather`:

- основные разделы открываются через slash-команды;
- действия внутри разделов выполняются через inline-кнопки и короткие wizard-сценарии;
- пошаговое состояние wizard сохраняется в SQLite, восстанавливается после рестарта процесса и может автоматически истекать по TTL.

## Требования

- Linux-сервер с `systemd`.
- `root`-доступ или `sudo`.
- Telegram-бот и токен от [@BotFather](https://t.me/BotFather).
- Telegram user ID администраторов (можно получить через `@userinfobot`).
- Публичный IPv4 сервера для `announce`.
- Домен для `tls_domain`.

## Быстрый старт (Linux)

Основной сценарий установки теперь - один bootstrap-скрипт, который:

- скачивает последние релизы `telemt` и `telemt-admin`;
- кладёт бинарники в `/usr/local/bin`;
- создаёт `telemt.toml`, `telemt-admin.toml`, `systemd` unit-файлы и Polkit-правило;
- включает и запускает `telemt.service` и `telemt-admin.service`.

Запуск:

```bash
curl -fsSL https://raw.githubusercontent.com/fgbm/telemt-admin/main/scripts/install.sh | sudo bash
```

Во время установки скрипт спросит только минимально необходимые значения:

- `bot_token`
- `admin_ids`
- `port` для `telemt` (по умолчанию `443`)
- `tls_domain`
- `announce` (публичный IPv4 сервера)

После завершения можно проверить статус:

```bash
sudo systemctl status telemt.service
sudo systemctl status telemt-admin.service
```

Скрипт генерирует минимальный `/etc/telemt.toml` такого вида:

```toml
[general]
use_middle_proxy = false

[general.modes]
classic = false
secure = false
tls = true

[server]
port = 443

[server.api]
enabled = true
listen = "127.0.0.1:9091"
whitelist = ["127.0.0.1/32", "::1/128"]
auth_header = "Bearer <generated-secret>"

[censorship]
tls_domain = "site.example"

[[server.listeners]]
ip = "0.0.0.0"
announce = "X.X.X.X"
```

## Ручная установка

Если хотите развернуть всё вручную без bootstrap-скрипта, ниже остаётся reference-сценарий.

### 1. Скачивание актуальной версии `telemt-admin`

```bash
curl -L -o telemt-admin.tar.gz https://github.com/fgbm/telemt-admin/releases/latest/download/telemt-admin-linux-x86_64.tar.gz && tar -xzf telemt-admin.tar.gz
```

### 2. Установка бинарного файла

```bash
sudo mv telemt-admin /usr/local/bin/
sudo chmod +x /usr/local/bin/telemt-admin
```

### 3. Минимальная конфигурация `telemt-admin`

Создайте файл `/etc/telemt-admin.toml`:

```toml
bot_token = "ВАШ_ТОКЕН_БОТА"
admin_ids = [123456789] # Ваш Telegram ID
telemt_config_path = "/etc/telemt.toml"
db_path = "/var/lib/telemt-admin/state.db"
service_name = "telemt.service"
users_page_size = 10

[security]
default_token_days = 14
max_token_days = 180
allow_auto_approve_tokens = true
wizard_state_ttl_seconds = 86400 # опционально: TTL wizard-state в секундах

[telemt_api]
enabled = true
base_url = "http://127.0.0.1:9091"
auth_header = "Bearer ВАШ_ТОЧНЫЙ_AUTH_HEADER"
timeout_ms = 5000
allow_file_fallback = true
prefer_api_links = true
```

> [!TIP]
> Параметр `bot_token` можно не указывать в конфиге, если переменная окружения `TELOXIDE_TOKEN` задана в окружении процесса.

## Установка как системного сервиса

Для надежной работы в фоновом режиме настройте `systemd`:

### 1. Создайте пользователя и директории

```bash
sudo useradd --system --home /var/lib/telemt-admin --shell /usr/sbin/nologin telemt-admin
sudo mkdir -p /var/lib/telemt-admin
sudo chown -R telemt-admin:telemt-admin /var/lib/telemt-admin
```

### 2. Создайте unit-файл `/etc/systemd/system/telemt-admin.service`

```ini
[Unit]
Description=telemt-admin Telegram Bot Service
After=network-online.target

[Service]
Type=simple
User=telemt-admin
Group=telemt-admin
WorkingDirectory=/var/lib/telemt-admin
ExecStart=/usr/local/bin/telemt-admin /etc/telemt-admin.toml
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

### 3. Настройте права доступа

Чтобы сервис мог управлять `telemt` и редактировать его конфиг без прав root:

#### А. Разрешите управление `telemt.service` через Polkit

Создайте файл `/etc/polkit-1/rules.d/50-telemt-admin.rules`:

```javascript
polkit.addRule(function(action, subject) {
    if (action.id == "org.freedesktop.systemd1.manage-units" &&
        action.lookup("unit") == "telemt.service" &&
        subject.user == "telemt-admin") {
        return polkit.Result.YES;
    }
});
```

Этого правила достаточно для команд `systemctl restart` и
`systemctl kill -s HUP --kill-who=main telemt.service`, которые использует бот.

#### Б. Настройте права на конфиг telemt

```bash
# Создаем группу telemt если её нет
sudo groupadd -f telemt

# Добавляем пользователя бота в группу
sudo usermod -aG telemt telemt-admin

# Меняем группу владельца конфига и даем права на запись группе
sudo chown :telemt /etc/telemt.toml
sudo chmod 664 /etc/telemt.toml
```

При такой настройке у пользователя `telemt-admin` есть право **читать fallback-конфиг** и при необходимости использовать legacy-путь записи в `/etc/telemt.toml`. В штатном режиме новые установки используют control API `telemt`, поэтому прямое редактирование файла больше не является основным механизмом.

### 4. Запустите сервис

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now telemt-admin.service
```

## Как пользоваться

### Для пользователей

1. Получить **пригласительный токен** от администратора.
2. Найти бота и нажать `/start`.
3. Ввести токен (или перейти по ссылке вида `https://t.me/MyBot?start=TOKEN`).
4. В зависимости от типа токена:
    - **Auto:** Бот сразу пришлет ссылку на прокси.
    - **Manual:** Бот создаст заявку ("Ожидайте подтверждения"), и после одобрения админом пришлет ссылку.

### Для администраторов

#### Управление заявками

При поступлении новой заявки (по Manual-токену) вы получите сообщение с кнопками:

- **✅ Одобрить**: генерация секрета, создание пользователя через control API `telemt`, сохранение локальной записи в БД, отправка ссылки пользователю. При сбое API возможен fallback на legacy-конфиг и `systemd`.
- **❌ Отклонить**: заявка отклоняется, пользователь получает уведомление.

В разделе `Заявки` доступен страничный список: новые заявки находятся выше, после открытия карточки можно вернуться на ту же страницу.

#### Управление токенами

Основной способ работы с токенами:

- `/token` — открыть inline-меню управления invite-токенами.
- В меню доступны создание токена и переход к списку активных токенов.
- Список токенов открывается с пагинацией и сортировкой по сроку действия: выше те, что истекают раньше.
- В списке токенов доступен поиск по коду.
- Отзыв токена выполняется из карточки конкретного токена.
- При создании токена бот возвращает готовую ссылку вида `https://t.me/MyBot?start=TOKEN` и код токена в моноширинном формате.

#### Админский интерфейс

После `/start` у администратора открывается главный inline-экран вместо постоянной reply-клавиатуры.
Основные разделы доступны через slash-команды:

- `/start` — открыть главный экран.
- `/user` — открыть раздел пользователей.
- `/service` — открыть компактную inline-панель управления `telemt.service` со статусом и последними событиями.
- `/token` — открыть inline-меню invite-токенов.
- `/link` — получить свою proxy-ссылку.
- `/help` — показать список команд.

В списках пользователей и токенов доступен поиск, а основные разделы админки построены по схеме: список -> карточка -> подтверждение -> назад.

Если backend-операция завершается ошибкой, администратор получает причину в отдельном отформатированном сообщении в чате.

В карточке пользователя доступны действия:

- `🔗 Данные + QR` — отправляет proxy-ссылку и QR-код для ручной пересылки пользователю.
- `⛔ Удалить пользователя` — удаляет пользователя из конфигурации `telemt` и деактивирует запись в БД.
- `⬅️ Назад к списку` — возвращает к той же странице пагинации.

В карточке токена доступны действия:

- `🗑 Отозвать токен` — деактивирует токен после подтверждения.
- `⬅️ Назад к списку` — возвращает к той же странице пагинации.

На сервисном экране рискованные действия (`Остановить`, `Перезапустить`) требуют подтверждения, а быстрые (`Обновить`, `Reload`) выполняются сразу.

**Основные команды:**

- `/start` — открыть главный экран.
- `/user` — открыть раздел пользователей.
- `/token` — открыть меню invite-токенов.
- `/service` — открыть панель управления сервисом.
- `/help` — показать справку.
- `/link` — получить свою proxy-ссылку; для администратора при отсутствии учётной записи доступ будет создан автоматически.

## Архитектурные решения и runbook

Для API-first интеграции с `telemt` дополнительно зафиксированы:

- `docs/adr/README.md` — индекс архитектурных решений;
- `docs/adr/001-telemt-api-backend.md` — выбор backend-модели `API-first + fallback`;
- `docs/adr/002-telemt-api-security-and-rollout.md` — security-правила и стратегия rollout;
- `docs/runbook.md` — пошаговое включение, проверка и откат integration path;
- `docs/backlog.md` — список следующих задач, включая observability и поддержку `/metrics`.

## Конфигурация (telemt-admin.toml)

- `bot_token` — токен бота от @BotFather (опционально, если есть `TELOXIDE_TOKEN`).
- `admin_ids` — массив ID администраторов `[123, 456]` (обязательный).
- `telemt_config_path` — путь к `/etc/telemt.toml` (default: `/etc/telemt.toml`).
- `db_path` — путь к `state.db` (default: `/var/lib/telemt-admin/state.db`).
- `service_name` — имя сервиса (default: `telemt.service`).
- `users_page_size` — размер страницы списков в админском интерфейсе, включая пользователей, invite-токены и заявки (default: `10`).
- `[telemt_api]` — настройки control API `telemt`:
  - `enabled` — включить API-first backend.
  - `base_url` — базовый URL control API, например `http://127.0.0.1:9091`.
  - `auth_header` — точное значение заголовка `Authorization`, которое ожидает `telemt`.
  - `timeout_ms` — таймаут HTTP-клиента.
  - `allow_file_fallback` — разрешить fallback на legacy-конфиг и `systemd`, если API недоступен.
  - `prefer_api_links` — приоритетно использовать ссылки, которые вернул control API.
- `[security]` — настройки безопасности токенов:
  - `default_token_days` — срок жизни токена по умолчанию (default: 14).
  - `max_token_days` — максимально допустимый срок (default: 180).
  - `allow_auto_approve_tokens` — разрешить создание auto-approve токенов (default: `true`).
  - `wizard_state_ttl_seconds` — опциональный TTL для сохранённого wizard-state в секундах. Если не задан, состояние хранится до явного завершения или отмены.

### Wizard-state и БД

- Пошаговое состояние wizard хранится в SQLite-таблице `bot_wizard_states`.
- Схема БД обновляется автоматически при старте бота.
- SQLite является источником истины для wizard-state, поэтому активный сценарий переживает рестарт процесса.
- Если задан `security.wizard_state_ttl_seconds`, просроченные wizard-state автоматически удаляются и не восстанавливаются после истечения TTL.

## Проверка после запуска

Проверьте, что сервис запустился и бот отвечает:

```bash
sudo systemctl status telemt-admin.service
sudo journalctl -u telemt-admin.service -n 50 --no-pager
```

Минимальный smoke-check:

1. Напишите боту `/start` с аккаунта администратора.
2. Убедитесь, что открывается главный inline-экран администратора.
3. Откройте `/token`, создайте токен через кнопки и активируйте его пользовательским аккаунтом.

## Обновление

### Обновление telemt (при установке быстрым скриптом)

```bash
curl -fsSL https://github.com/telemt/telemt/releases/latest/download/telemt-$(uname -m)-linux-gnu.tar.gz | sudo tar -xz -C /usr/local/bin/ && sudo systemctl restart telemt.service
```

### Проверка и автообновление telemt-admin

Проверить наличие новой версии:

```bash
telemt-admin check-update
```

На Linux x86_64 доступно автообновление (требуются права на запись в директорию с бинарником):

```bash
sudo telemt-admin self-update
```

Если бинарник установлен в `/usr/local/bin/` и сервис запущен от пользователя `telemt-admin`, для автообновления потребуется выполнить команду от root. После успешного обновления перезапустите сервис:

```bash
sudo systemctl restart telemt-admin.service
```

На Windows и других платформах автообновление не поддерживается — используйте ручное скачивание из раздела «Быстрый старт».

## Сборка из исходников

Если вы хотите собрать бота самостоятельно:

1. Установите Rust (Edition 2024).
2. Выполните сборку: `cargo build --release`.
3. Бинарный файл будет находиться в `target/release/telemt-admin`.

### CLI

- `telemt-admin --help` / `-h` — справка по аргументам.
- `telemt-admin --version` / `-V` — версия.
- `telemt-admin -c <FILE>` / `--config <FILE>` — путь к конфигу.
- `telemt-admin check-update` — проверить наличие новой версии.
- `telemt-admin self-update` — автообновление (только Linux x86_64).

Запуск с кастомным конфигом (оба варианта поддерживаются):

```bash
./target/release/telemt-admin /path/to/telemt-admin.toml
./target/release/telemt-admin --config /path/to/telemt-admin.toml
```

## Troubleshooting

- `Не задан bot_token...`  
  Укажите `bot_token` в конфиге или задайте `TELOXIDE_TOKEN` в окружении сервиса.

- `telemt API error ...`  
  Проверьте, что в `telemt` включён `[server.api]`, bind доступен с машины бота, а `telemt_api.auth_header` в `telemt-admin.toml` в точности совпадает с `server.api.auth_header`.

- `Permission denied` при записи в `/etc/telemt.toml`  
  Это важно только для fallback-режима. Проверьте группу/права файла и что пользователь `telemt-admin` входит в нужную группу `telemt`.

- Не удаётся выполнить `/service restart` или автоматическое применение конфига  
  Проверьте правило Polkit для `org.freedesktop.systemd1.manage-units`, корректность `service_name` и что у пользователя сервиса есть доступ к `telemt.service`.

- Бот не отвечает на команды  
  Проверьте логи `journalctl -u telemt-admin.service` и валидность токена бота.

## CI/CD

Проект использует GitHub Actions для автоматической проверки кода (`clippy`, `check`) и сборки релизов под Linux и Windows при создании тега версии (`vX.Y.Z`).
