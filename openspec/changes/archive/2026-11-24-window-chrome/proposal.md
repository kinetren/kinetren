# Proposal: window-chrome

> Built 2026-11-24 (features-slow); revision 2 (2026-11-22): the owner's B9 drops the drag by the preview. Registry `v26.10-index.md` U10, U11, T5, T7, T8, T9, W1, W3 (round R.76).
> Tests reserved: t3935–t3942.

## Why

The owner's `v26.10` list, the window items (verbatim in `docs/ktprompts/v26.10.txt`):
- line 36 (U10): "для окна с маленьким размером по ширине хотелось бы возможность скролить тулбар влева-вправо
  колесиком мышки";
- line 37 (U11): "окно превью видео хотелось бы перетаскивать хватаясь прямо за него + дизайн титлбара у окна
  сделать такой же как у основного окна. и основное окно тоже хочется перетаскивать даже хватаясь за превью";
- line 47 (T5): a window started maximized and dragged by its title moves at the same size; maximized again and
  dragged, "оно наконецто чуть-чуть уменьшится … значит код говно";
- line 49 (T7): "все кнопки копирования-вставки … подсократить текст, иконки сделать";
- line 50 (T8): "в ui эффектов при разворачивании аудио/видео у них есть боковая полоска слева … ее бы нафиг выкинуть";
- line 51 (T9): "если ui эффектов подвинуть в самый верх окна, то на заголовке ui окна пролезает надпись
  именя_проекта+kt+версия_программы";
- lines 62–64 (W1): the glow of the preview and the meters shows through an opaque window, and only partly ("тело
  не просвечивается … можно упростить и сделать всё на одном объекте? … починить чтобы не просвечивало");
- line 66 (W3): "в заголовке что-то по делу отображалось вместо абстрактного номера события. Но номер события нужно
  хотя бы в самом окне где-то непринужденно отображать".

Two of these share one mechanism: **the main window's own drawing is painted on egui's `Foreground` order, which
lies above every tool window** (`app.rs`: the "ambient-overlay" glow layer, the "status-hint" layer, the title text).
A window moved over them shows them through itself (W1, T9). The others are separate small defects of the same
round.

## What Changes

- **Nothing of the main window is painted over a tool window** (W1, T9): the glow of the preview and the meters
  becomes ONE mesh per glowing thing in one layer at the bottom of the window stack (above the panels, below every
  window); the title text and the status hint move under the windows too. A guard lists every `Foreground` layer
  of the program with its reason (tooltips, drag ghosts, the lesson outline, popups).
- **A window started maximized drags like one** (T5): the maximized state is read from the window manager at the
  start; restoring a window that never had a normal size gives it 80 % of the monitor's work area centred, before the
  drag starts.
- **The toolbar scrolls with the wheel when it does not fit** (U10): over the toolbar row the wheel (and Shift+wheel)
  moves the row sideways; the "more" menu of groups that do not fit stays.
- **The detached preview's title row** is the shell's (change `detached-windows`), the same as the main window's (U11's
  second half). Dragging a window by the preview picture is dropped (the owner, B9).
- **The effects window's folds have no side strip** (T8): the per-chain fold is drawn unindented.
- **Copy and paste buttons are icons** (T7): Copy the chain, Paste the chain, Copy the envelope, Paste the envelope
  become icon buttons with their hints (the head menu of change `presets-everywhere` replaces them where it lands).
- **An event window's title says which event** (W3): "<event name> — Effects" (the event's source file name, a
  generator's kind or a title's first words, a reel's name), a pair "<name> + sound — Effects"; the event's number is
  written small in the window's chain head row, where there is room.

## Capabilities

### Modified Capabilities

- `ui-timeline`: the layer order of the main window's drawing; the glow as one object under the windows; the
  maximized start; the toolbar's wheel.
- `ui-reach`: the event windows' titles and the number in the body; icon copy/paste buttons; unindented folds.

## Impact

- kt-app: `app.rs` (the ambient overlay and the status hint layers, the title), `meters.rs`/`theme.rs` (one glow
  mesh), `toolbar.rs` (wheel offset; `window_drag_from` restoring a maximized start), `window_frame.rs` (the WM's
  maximized state at start), `fx_editor.rs` (titles, unindented folds, icon buttons), `node_editor.rs` (icon
  buttons), i18n ×5 (titles).
- No model, engine or format change.
- Idle: no new work; the glow is the same meshes, drawn in another layer.

## Questions to the owner

Decided (2026-11-22, B9, `kkt-26.11.20b.txt` line 21): dragging a window by the preview — "пропускаем это требование."
No open question.
