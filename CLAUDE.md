# Kinotrain

Нелинейный видеоредактор. Кодовая база — cargo workspace в **`src/`**.

Переписывание с C++/Qt (v1) завершено с полным паритетом; старый код удалён из рабочего дерева и доступен в git-теге **`cpp-reference-final`** (сверка поведения: `git show cpp-reference-final:<файл>`). `concepts/` — исторические заметки о проблемах v1 (русский; пути к файлам указывают в тег); актуальный каталог дефектов — `docs/porting-notes.md`. Архитектура движка: `docs/architecture.md`.

## Сборка и тесты (Rust)

```bash
cd src
cargo test                 # быстрые синтетические тесты (без FFmpeg/аудио-железа)
cargo test --features conformance   # + conformance на реальных фикстурах (нужен ffmpeg-бинарник)
cargo clippy --all-targets
cargo run -p kt-app        # GUI
cargo run -p kt-cli -- …   # headless-рендер/экспорт
./scripts/release-build.sh # релизный тарбол: dist-профиль (strip+LTO) в контейнере
                           # Ubuntu 22.04 → работает на glibc >= 2.35; внутри
                           # проверяются «ldd без libav*» и потолок GLIBC_
```

Системные требования для полной сборки (kt-media/kt-app): dev-пакеты FFmpeg (libavcodec/format/util/swscale/swresample), `clang` (bindgen), `libasound2-dev` (cpal/ALSA), бинарник `ffmpeg` в PATH (экспорт и генерация фикстур). Крейты kt-time/kt-model/kt-fx/kt-cache и ядро kt-engine собираются и тестируются **без** всего этого.


## Карта workspace

```
src/apps/kt-app     eframe GUI: таймлайн, превью, cpal-вывод
src/apps/kt-cli     headless-рендер (детерминизм, golden-тесты, CI)
src/libs/kt-time    тики 90 kHz, диапазоны, сетки fps/аудио-burst (без зависимостей)
src/libs/kt-model   модель таймлайна, команды, undo, формат проекта (.ktproj)
src/libs/kt-media   трейт MediaSource, мост к kt-ffmpeg, SyntheticSource, sidecar индекс-БД
src/libs/kt-cache   кэш burst'ов + coverage (единственная правда о «прогретости»)
src/libs/kt-fx      AudioFx/VideoFx: slowedreverb, gain, видео-FX
src/libs/kt-engine  render graph, MediaWorker-планировщик, transport, prefetch, экспорт
```

В каждом libs/-крейте есть свой `CLAUDE.md` с инвариантами и неочевидными решениями. Читай его перед правкой крейта.

Состояние UI-паритета со старым Qt-редактором (инвентаризация в docs/porting-notes.md): U1–U4 выполнены — маркеры/регион/loop/punch-out/копипаст-цепочки/volume rail/стерео-волноформы/филмстрип/метры+фейдер/медиа-бин/FX-редактор с автоматизацией/рендер-диалог/автосейв. E5 перенёс все 10 видео-FX, H1 — morpher-систему (morph-дорожки, размещение, FX-батчи, bake; тесты 35–36). I1 довёл перф-панель до уровня v1 и выше: 5-серийный график FFmpeg-активности (Total/Open/Read/VDec/ADec по живым счётчикам kt-ffmpeg::iostats), no-read drops (prefetch-thrash) в кэш-чёрне, занятость кэша от настоящего лимита, «pinned legends» с выносками, дедуп журнала «×N», пороговые предупреждения. Функциональный паритет с v1 ЗАКРЫТ (в v1 треть метрик панели были мёртвыми нулями — их не воспроизводим, см. тест 37).

## Модель времени

- Единица — **тик = 1/90000 c** (`kt_time::Tick`). 90 kHz делится на все частоты кадров (24/25/30/60) и на шаг аудио-burst.
- Аудио: **48 kHz stereo f32 interleaved** сразу после декода и до самого выхода. Аудио-burst = 1024 кадра = 1920 тиков ≈ 21.3 мс. Тик↔сэмпл: `samples = ticks * 48000 / 90000` — точно (15/8).
- Видео: кадр по сетке fps проекта; burst-индекс выравнивается на шаг `90000/fps`.
- В формате проекта — только тики, никаких миллисекунд.

## Инварианты (нарушение любого — регрессия; у каждого есть тест)

1. **Аудио внутри движка всегда f32 48kHz/2ch.** FX и микс никогда не видят исходный sample-rate файла. (Дефект старого движка: warp-путь считал в rate источника → периодические щелчки.)
2. **FX-состояние трогает только аудио-render-поток.** Никаких мьютексов вокруг FX, никакого доступа из prefetch/метров. Prefetch наполняет только пре-FX кэш. (Старый движок: 4 потока без локов по общему delay line.)
3. **Правка модели не уничтожает FX-состояние.** Diff цепочек по clip id: те же цепочки → те же state-объекты; параметры → `set_params` со сглаживанием; буферы зануляются только в `reset()` при seek. (Старый rebuildGraph убивал хвосты реверба при любом чихе UI.)
4. **Кэш — единственная правда о coverage.** Полоски «прогретости» в UI — это `BurstCache::coverage()`, не отдельная бухгалтерия. Quality — часть ключа кэша.
5. **Декодером владеет ровно один MediaWorker-поток.** Все клиенты (playback/prefetch/тумбнейлы) ходят через его приоритетную очередь (Playback > NearPrefetch > FarPrefetch > Thumbnail) с generation-отменой. Playback не блокируется: нет данных к дедлайну → тишина/последний кадр + underrun-флаг.
6. **cpal-колбэк — мастер-часы.** Playhead продвигается ровно на потреблённые колбэком кадры. В колбэке ни аллокаций, ни локов — только чтение SPSC-кольца. Видео подчинено аудио-часам.
7. **Путь рендеринга один.** Экспорт = офлайн-прогон того же render graph, что и превью (другой clock и Quality — тот же код композиции). Закреплено golden-тестом превью==экспорт.
8. **Скролл/зум таймлайна не порождает декод-запросов.** Волноформы — из peak-пирамиды, тумбнейлы — из полосы в sidecar индекс-БД (`<project>.ktindex.db`). Пока индекса нет — плейсхолдеры.
9. **Экспорт полностью внутрипроцессный.** Мукс и кодирование — через libavformat/libavcodec СИСТЕМНОГО FFmpeg (kt-ffmpeg::Muxer): ни временных файлов, ни внешнего бинарника `ffmpeg`, ни mkfifo. Видео-энкодер выбирается автоматически с реальной проверкой открытия: h264_nvenc → h264_vaapi → h264_qsv → libx264 → mpeg4 (наличие энкодера в сборке не значит, что он заработает — nvenc без драйвера падает на avcodec_open2, тогда берётся следующий).
10. **Бинарь не линкуется с libav*.** FFmpeg грузится в рантайме через dlopen (kt-ffmpeg): мажор определяется по `avcodec_version()`, типы структур берутся из версионного модуля (bindgen по вендоренным заголовкам `libs/kt-ffmpeg/vendor/ffmpeg-{6,7,8}`). Поэтому один и тот же исполняемый файл работает на системах с FFmpeg 6/7/8 и использует все аппаратные ускорения СИСТЕМНОГО FFmpeg. Проверка: `ldd target/release/kt-app | grep -c libav` == 0.
11. **Детерминизм.** Один и тот же проект → байт-в-байт тот же рендер (seek с pre-roll `max(tail, 100 мс)`, никакого wall-clock в офлайн-путях). Тесты сравнивают точные значения, не epsilon.

## Правила для агентов

- Перед правкой крейта — его `CLAUDE.md`; перед архитектурными изменениями — `docs/architecture.md`.
- Новый найденный дефект/тонкость → сразу документировать (`docs/porting-notes.md` или CLAUDE.md крейта) и по возможности закрыть регрессионным тестом.
- `cargo test` обязан быть зелёным на каждый коммит; тесты 1–40 из `docs/porting-notes.md` — регрессионные, их нельзя ослаблять/удалять без явного решения пользователя.
- Не добавлять зависимость от FFmpeg/аудио-железа в kt-time/kt-model/kt-fx/kt-cache/ядро kt-engine.

## Карта крейт → обязательные тесты (гонять при правках, требование пользователя)

| Правишь | Обязательно зелёные |
|---|---|
| kt-fx (аудио) | kt-engine/tests/{fx (вкл. t31),warp_prefetch,reverse}.rs, golden_export.rs, kt-fx unit (source-карта) |
| kt-fx/video.rs | kt-fx/tests/video_fx.rs (golden 26), kt-engine/tests/reverse.rs::t26 |
| kt-engine/audio.rs | audio_engine, warp_prefetch, splice, reverse (вкл. t29), golden_export |
| kt-engine/worker.rs | scheduler, video_span, video_stall (t27/t28) |
| kt-engine/player.rs | player_dynamic (t18/t20), audio_engine::t14, unit t22-sync/t32-varispeed |
| kt-engine/video.rs | viewport, reverse (t24/t26), golden_export |
| kt-engine/export.rs | golden_export (11), export_options (25, 33) |
| kt-ffmpeg | kt-media/tests/ffmpeg_conformance.rs (t10/t21/ogg/t34/t37), export_options t33 (муксер) |
| kt-cache | kt-cache unit (t8/t14/t27/t28/t37 no-read drops) |
| kt-media | ffmpeg_conformance (вкл. t34 — персист пакетного индекса), index_db, kt-app unit t23 |
| kt-model | model_roundtrip, parity_ops (вкл. t24/t29/t30), morpher t35 |
| kt-engine (morpher-flatten) | morpher_render t36 (рендер с morpher == bake) |
| kt-app (util/thumbs) | kt-app unit-тесты (t20-repaint, t23, регион-плей) |

Всё вместе: `cd src && cargo test --workspace` (плюс `cargo clippy`).