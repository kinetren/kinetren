# Kinetren — new feature-request prompts (draft, 2026-11-01)

Not repeated: VHS, camera ISP, datamosh, multicam, point tracker, classic and magic-wand masks, nor what R.72–R.78 and the open changes already take. Web: Resolve 20, FCP 11, Premiere/AE 26.x, Vegas 23, CapCut desktop release notes (searched 2026-11-01). Reddit threads did not surface, so "what people complain about" rests on my own knowledge. ★ = top 5.

## Editing

**1. ★ Ripple и режимы подрезки.** Нет ripple — после каждого удалённого куска интервью я руками тащу весь хвост. Хочу как в Vegas: auto-ripple кнопкой (трек / все треки) с красным индикатором, ripple-delete, roll, slip, slide, «закрыть все дыры на треке», и ножницы по региону с клавиатуры, которые сразу схлопывают дырку.
*Source: F-TL-001 (A), F-TL-002, F-TL-008, ktprompts v26.10 U9. Covered: no ripple; slip partial. Why: the edit a dialogue cutter makes dozens of times an hour.*

**2. Исходный монитор и трёхточечный монтаж.** Хочу открыть сурс в своём мониторе, поставить In/Out и одной клавишей кинуть кусок на таймлайн в плейхед — вставкой со сдвигом или поверх. И обратно: с события прыгнуть в тот же кадр сурса (match frame), заменить событие другим дублем той же длины.
*Source: no note (Vegas Trimmer, Premiere Source Monitor); next to ktprompts MW1. Covered: bin scrub only. Why: picking from long material without dragging whole files.*

**3. Детектор склеек.** Приносят готовый ролик или запись стрима — хочу «найти склейки»: прогон в фоне, сначала маркеры на сменах сцен с порогом, просмотрел — порезал одним Undo.
*Source: no note (Resolve Scene Cut Detector, Premiere Scene Edit Detection); ktprompts N5 (MD-SAD). Covered: nothing; home is the sidecar offline-analysis pass. Why: re-editing someone else's render.*

## Audio

**4. Запись войсовера в таймлайн.** Хочу писать голос с микрофона прямо на трек: выбрал вход, отсчёт, punch-in по региону, каждый проход — отдельный дубль, потом выбираю лучший; уровень входа на тех же метрах.
*Source: no note; Resolve 20 voice-over palette. Covered: no input stream in the code (cpal output only). Why: narration and tutorials are recorded into the edit.*

**5. Шумодав.** Звук с телефона и петлички всегда с шумом, гулом 50 Гц и щелчками. Выделил кусок тишины — «запомнить шум», убрать; отдельно де-хум и де-клик. Анализ в фоне, как пики, плейбек не страдает.
*Source: F-AUD-003 (candidate: spectral reducer as the offline-analysis pilot). Covered: no. Why: the most common audio rescue.*

**6. Выровнять громкость событий.** Три человека и музыка, все на разной громкости. Хочу выделить события и «выровнять к −16 LUFS» или «как у этого», gain пишется в событие и виден бейджем; шаблоны YouTube/подкаст/эфир.
*Source: F-AUD-005; Premiere Essential Sound, Resolve 20 Audio Assistant. Covered: LUFS measured at export; the spec forbids a master "normalize", event gain is the allowed place. Why: uneven speakers are the first thing viewers hear.*

**7. Несколько аудиодорожек в файле.** OBS пишет микрофон и игру отдельными дорожками в один mkv, а берётся одна. Хочу при импорте видеть все и класть каждую на свой трек, и раскладку каналов: две петлички из одного стерео — в два события.
*Source: F-MED-006, F-AUD-006. Covered: no; the stream number is not stored (an invariant 11 hole). Why: streamers and podcasts deliver exactly this.*

## Colour

**8. ★ Скопы.** Цвет кручу на глаз на некалиброванном мониторе. Нужны waveform, RGB-parade, вектороскоп, гистограмма и false color по кадру превью или по событию.
*Source: F-CLR-005 ("first in the colour branch"). Covered: no. Why: zero risk to invariants 7/11, every colour tool needs it.*

**9. Нормальная цветокоррекция.** Хочу колёса Lift/Gamma/Gain/Offset, RGB- и Luma-кривые, «оттенок против насыщенности», пипетку баланса белого и LUT `.cube` с силой.
*Source: F-CLR-001, F-CLR-002, F-CLR-004. Covered: ColorGrade/Duotone/Posterize only. Why: the decade-old minimum of every NLE.*

**10. Лог, HDR и 10 бит.** Айфон снимает HLG и Apple Log — у нас картинка блеклая. Хочу, чтобы читалось цветовое пространство файла, был один рабочий, а экспорт — 10-бит HEVC/HDR10 с тегами или честный тон-маппинг в SDR.
*Source: F-CLR-008, ktprompts v26.10 FUT "HDR10". Covered: no. Why: phones shoot HDR by default.*

## Motion / compositing

**11. Стабилизация.** На тряску с рук хочу «стабилизировать»: анализ в фоне, плавность, кроп или без, режим «штатив». Анализ — общий с трекером, который я уже просил.
*Source: F-MSK-006 (after F-MSK-005). Covered: no. Why: handheld footage is most of what people bring.*

**12. Optical flow и motion blur.** Смешивание кадров есть, но ниже 10–20 % замедление двоит. Нужен optical flow (fruc) и motion blur от движения геометрии и камеры, как у слоёв в AE, с углом затвора.
*Source: F-SPD-003, ktprompts FUT "fruc_vulkan". Covered: frame repeat and blending (R.37). Why: speed ramps and fast moves look cheap without it; needs a decision on invariants 7/11.*

**13. Трек-матт.** Хочу, чтобы событие задавало прозрачность другого: текст как окно в видео, по луме или альфе, с инверсией и жестом «верхний — трафарет».
*Source: F-MSK-003 (luma matte by explicit clip reference). Covered: no; alpha waits on F-MED-003. Why: text reveals and wipes are built on mattes.*

## Titles

**14. ★ Субтитры и монтаж по тексту.** Нужна дорожка субтитров: SRT/VTT туда-обратно, вшивка стилем текста, анимация по словам как в CapCut. Распознавание — whisper из FFmpeg 8, модель качается по клику как сам FFmpeg; в панели текста ищу фразу, удаляю слова — режется видео, паузы и «эээ» — маркерами.
*Source: F-TXT-004, F-AUD-008, ktprompts N4; FCP 11 Transcribe to Captions, CapCut Auto Captions. Covered: rasteriser ready (F-TXT-001); the note rejected bundled weights, so this revises it. Why: most social video plays muted.*

## Export / delivery

**15. ★ Один проект — несколько форматов кадра.** Сделал 16:9, нужен ещё 9:16 и квадрат. Хочу варианты кадра одного таймлайна: у каждого своя поправка геометрии события поверх общей, safe-area сетки, в рендере «все форматы сразу».
*Source: no note; FCP Smart Conform, Premiere/CapCut Auto Reframe. Covered: frame rule and per-event Geometry, no per-format layer. Why: every piece also goes to Shorts/Reels/TikTok.*

**16. Очередь рендера.** Хочу накидать в очередь рендеры (регионы, форматы, проекты) и монтировать дальше, пока они идут в фоне ниже плейбека; потом список — что вышло, что упало.
*Source: F-EXP-002 (internal queue). Covered: one render from the dialog. Why: delivery means several files.*

**17. Рендер на месте.** Тяжёлую композицию хочу «заморозить»: отрендерить в файл рядом с проектом, подменить, потом одной кнопкой вернуть живое — как Render in Place в Resolve.
*Source: no note; Vegas, Premiere Render and Replace, Resolve. Covered: reel previews have a disk tier, nothing pins an event. Why: the standard answer to a spot that will not play in real time.*

## Workflow / UI

**18. ★ Прокси.** 4K HEVC 10 бит с телефона тормозит на скрабе. Хочу «сделать прокси» в фоне, переключатель прокси/оригинал одним кликом, индикатор, а рендер — всегда с оригиналов.
*Source: F-MED-002; half exists (`Player::replace_media`, sidecar DB, muxer). Covered: partial. Why: long-GOP 10-bit HEVC is the default phone format.*

## Collaboration / media management

**19. Перепривязать и собрать проект.** Перенёс проект на другой диск — всё красное. Хочу: указал одну папку, остальное нашлось по имени, размеру, хешу; и «собрать проект в папку» со всеми сурсами.
*Source: F-MED-005 (`MediaRef::path` is immutable). Covered: no. Why: every handoff breaks paths.*

**20. Ревью-копия и правки обратно.** Отдаю черновик заказчику — рендер с вшитым таймкодом и версией, а его правки (таймкод + текст, хоть из почты) вставляю обратно маркерами с заметками на кадрах.
*Source: F-COL-025, F-COL-026, ktprompts U8. Covered: markers yes, text notes no. Why: client review is the loop of every paid edit, offline.*
