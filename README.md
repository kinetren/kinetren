# Kinotrain

Нелинейный видеоредактор.

- **`src/`** — актуальная кодовая база (Rust: egui + rsmpeg/FFmpeg + cpal). См. `src/README` в крейтах и корневой `CLAUDE.md`.
- Корень репозитория и `src/burster/` — замороженная C++/Qt-версия, оставлена как поведенческий reference до достижения паритета.

Документация: [CLAUDE.md](CLAUDE.md) (карта проекта, инварианты), [docs/architecture.md](docs/architecture.md), [docs/porting-notes.md](docs/porting-notes.md) (каталог дефектов старого движка и регрессионные тесты), [docs/file-format.md](docs/file-format.md), [docs/media-index.md](docs/media-index.md).

```bash
cd src
cargo test          # тесты движка (без FFmpeg/аудио-железа)
cargo run -p kt-app # GUI
```
