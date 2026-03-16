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
- `.github/workflows/release.yml` собирает релизы для Linux и Windows по тегам `v*.*.*`;
- changelog формируется через `git-cliff` и Conventional Commits.

Конвенции:

- предпочтительный стиль коммитов: `feat:`, `fix:`, `refactor:`, `docs:` и т.д.;
- не писать тесты без явного запроса;
- не добавлять зависимости без явной необходимости;
- не использовать `unwrap()` там, где ошибка может быть штатной или внешней;
- при изменениях bot UX проверять вручную не только wizard-flow, но и legacy slash-команды с аргументами, а также восстановление wizard-state после рестарта.
