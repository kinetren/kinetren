# Чужие .otio-сэмплы (кросс-рендер конформанс, тест 158)

Копии `tests/sample_data` из апстрима OpenTimelineIO
(github.com/AcademySoftwareFoundation/OpenTimelineIO), лицензия Apache-2.0 —
текст в `LICENSE` рядом.

Пин: **73803eff1dbf86b82bcf97179567a243f4036b90** (2026-06-23).
Обновление: `src/scripts/fetch-otio-samples.sh` (поправить `PIN` там и здесь);
дрейф апстрима виден обычным `git diff`.

Файлы коммитятся намеренно (решение пользователя Р.25 п.2): «из интернета» —
про происхождение материала, а не про способ доставки; тест обязан работать
офлайн и детерминированно.

## Что в них ценного (почему выбраны именно эти)

| Файл | Конструкции |
|---|---|
| `simple_cut.otio` | 4 склейки, ExternalReference; **Clip-004 без `source_range`** — длительность берётся из `available_range` |
| `multiple_track.otio` | 3 дорожки, Gap'ы, ExternalReference |
| `multitrack.otio` | 3 дорожки, 7 Gap'ов, MissingReference |
| `preflattened.otio` | одна дорожка, Gap'ы — эталон «сплющенного» монтажа |
| `transition_test.otio` | 3 SMPTE_Dissolve, в том числе **у края дорожки** (fade-in), клипы с `media_reference: null` |
| `screening_example.otio` | EDL-стиль, MissingReference, `Marker.3` |
| `nested_example.otio` | **вложенные Stack**, `LinearTimeWarp`, `FreezeFrame` |
| `premiere_example.otio` | экспорт Premiere: `Marker.3`/`Color.1`/`GeneratorReference`, rate 15 и 30 |
| `generator_reference_test.otio` | архаичная схема `Sequence.1` (предок `Track`) |

Синтетические исходники под них НЕ коммитятся: их генерирует
`src/scripts/otio-cross-prepare.py` (сплошные цвета по палитре) во временный
каталог теста.
