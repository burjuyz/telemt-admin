# Разработка

Локальная проверка после заметных изменений:

```bash
cargo check --locked
cargo clippy --all-targets -- -D warnings
```

Если менялись зависимости:

```bash
cargo check
```

CI/CD:

- `.github/workflows/ci.yml` проверяет `cargo check --locked` и `cargo clippy --all-targets -- -D warnings`;
- `.github/workflows/release.yml` собирает релизы для Linux и Windows по тегам `v*.*.*` и публикует Docker-образ в GHCR (`ghcr.io/<owner>/telemt-admin`);
- changelog формируется через `git-cliff` и Conventional Commits.
- `scripts/install.sh` ориентирован на Linux x86_64 (glibc) + `systemd` и скачивает release-артефакты из GitHub.
- контейнерная сборка: корневой `Dockerfile`, пример `deploy/compose/docker-compose.telemt-admin.example.yml`;
- overlay конфигурации через whitelist `TELEMT_ADMIN__*` (см. `docs/adr/004-config-sources-and-docker-defaults.md`).

Конвенции:

- предпочтительный стиль коммитов: `feat:`, `fix:`, `refactor:`, `docs:` и т.д.;
- не писать тесты без явного запроса;
- не добавлять зависимости без явной необходимости;
- не использовать `unwrap()` там, где ошибка может быть штатной или внешней;
- если меняются архитектурные границы или модульная структура, синхронно обновлять `docs/*` и `.cursor/rules/*` в той же серии изменений;
- если меняются инварианты интеграции с `telemt`, стратегия fallback, rollout или security-модель, обновлять соответствующий ADR в `docs/adr/` или добавлять новый;
- если меняется эксплуатационное поведение control API, синхронно обновлять `docs/runbook.md`;
- если меняются notifications, health/runtime alerts или фоновый polling, синхронно обновлять `README.md`, `docs/overview.md`, `docs/architecture.md` и `docs/runbook.md`;
- при изменениях bot UX проверять вручную wizard-flow, основные inline-экраны и восстановление wizard-state после рестарта.

Навигация по документации:

- `docs/overview.md` — краткая целевая модель проекта;
- `docs/architecture.md` — модули и границы слоёв;
- `docs/adr/README.md` — индекс архитектурных решений;
- `docs/runbook.md` — rollout, проверка и откат API-first интеграции;
- `docs/backlog.md` — следующий слой задач для реализации и наблюдаемости.
