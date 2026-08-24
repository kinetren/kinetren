# Kinotrain

Нелинейный видеоредактор (Rust: egui + системный FFmpeg через dlopen + cpal).

Кодовая база — cargo workspace в **`src/`**. Документация: [CLAUDE.md](CLAUDE.md) (карта проекта, инварианты), [docs/architecture.md](docs/architecture.md), [docs/porting-notes.md](docs/porting-notes.md) (каталог дефектов старого движка и регрессионные тесты 1–37), [docs/file-format.md](docs/file-format.md), [docs/media-index.md](docs/media-index.md).

```bash
cd src
cargo test                 # тесты движка (без FFmpeg/аудио-железа)
cargo run -p kt-app        # GUI
cargo run -p kt-cli -- …   # headless-рендер/экспорт
./scripts/release-build.sh # релизный тарбол (контейнер Ubuntu 22.04, glibc >= 2.35)
```

Историческая C++/Qt-версия (v1), служившая поведенческим эталоном при переписывании, удалена из рабочего дерева после достижения полного паритета — доступна в git-теге `cpp-reference-final`. Заметки о её дефектах: `concepts/` и `docs/porting-notes.md`.
