# kt-media

Источники медиа: трейт `MediaSource`, `SyntheticSource` (тесты), `FfmpegSource` (rsmpeg, feature `ffmpeg`, M3), sidecar индекс-БД (M3, схема — docs/media-index.md).

## Инварианты
- Всё аудио наружу — **48 kHz stereo f32 interleaved**, ровно 1024 кадра на burst. Приведение форматов (swr) происходит ВНУТРИ источника. Ни один потребитель не видит sample-rate файла (дефект a — хрипы slowed&reverb в старом коде).
- `read_audio_burst(idx)` sample-accurate: burst idx покрывает сэмплы [idx·1024, (idx+1)·1024) от начала аудиопотока. Проверяется conformance-тестом 10 (== декод с нуля).
- Экземпляром `MediaSource` владеет один поток (`&mut self`) — шедулер kt-engine, не сам крейт, гарантирует эксклюзивность.
- `SyntheticSource` живёт в основном крейте (не под cfg(test)) и считает обращения (`audio_reads`/`video_reads`) — на этом строятся регрессии «ноль декодов» (тест 13, 15).
- rsmpeg-специфика не выходит за пределы крейта; смена FFmpeg-мажора не трогает остальной код.

## Тонкости для FfmpegSource (M3, из C++ опыта)
- `threads=1` для VP9/AV1 (старый код форсил — иначе seek нестабилен).
- Backward seek при многопоточном H.264 может требовать полного переоткрытия декодера (bc_avcpp_decoder.cpp:1160-1168) — сохранить fallback.
- Аудио seek-индекс (позиция пакета → первый сэмпл) строится один раз и персистится в индекс-БД; никаких «грубых» seek с decode-from-reopen (дефект j).
- Системный FFmpeg: на этой машине 6.1 (runtime), dev-заголовков НЕТ — до M3 нужно установить libav*-dev, clang; rsmpeg-версию/фичу подобрать под 6.x.
