# FastCloud Airwave — концепция редизайна и план реализации

> **Актуальный статус, 16 сентября 2026:** редизайн не закончен. Идёт второй
> визуальный проход: прозрачность поверх пользовательских обоев, композиция
> оболочки и проверка каждого состояния в полном окне. Старые отметки ниже означают наличие реализации, а
> не финальное качество интерфейса.

## Чек-лист второго визуального прохода

- [x] AppShell: широкая навигация на desktop, атмосферный фон Airwave, три уровня поверхностей;
- [x] glass material system: полупрозрачные semantic surface-роли, внутренний блик, edge-light и отдельная глубина плавающих панелей;
- [x] второй проход Sidebar, Settings-nav и Quick Picks: компактнее геометрия, стеклянные active-state и неодинаковые feature-карточки;
- [x] третья ревизия оболочки по пользовательскому сравнению: полноширинный titlebar, низкий поиск, прозрачный sidebar с профилем снизу и без декоративной плашки;
- [x] Sidebar занимает реальную высоту viewport: нижний player резервирует место только в центральной области; Quick access использует свободное место для текущей очереди;
- [x] Quick Picks занимает три широкие колонки при 1280 px; Home/Discover/Settings используют прозрачный слой поверх обоев;
- [x] Settings: неподвижная навигация по разделам и отдельная прокрутка содержимого без пустого левого столбца, активный раздел следует за прокруткой;
- [x] плавающая player deck с собственной глубиной вместо плоской нижней полосы;
- [x] Settings: отдельная навигация, hero и карточки групп настроек;
- [x] Feed: стеклянный live-signal hero, публикации с edge-light, адаптивная правая колонка и узкий layout;
- [x] Library: единый archive hero, интерактивный soundprint и отдельные состояния Overview, Likes, Playlists, Albums, Stations, Following, History;
- [x] Discover: редакционный glass-hero, визуальный signal preview, Vibe-ввод и персональная волна;
- [x] Search: единая стеклянная панель запроса/режимов, контекстный preview signal и мягкое выделение результата;
- [x] Track, Playlist и Artist detail: общий media-hero с ambient-подсветкой, стеклянные панели управления и контента;
- [x] golden-проверки Home, Discover, Search, Feed, Settings и всех вкладок Library;
- [x] golden-проверки Recent, Track, Playlist и Artist detail;
- [x] узкие golden-кадры Discover, Search и трёх detail-маршрутов при 760 × 800;
- [x] golden-кадры Home/Settings с реальным локальным изображением в качестве обоев и Home после прокрутки до Recently played;
- [ ] финальная полировка Home и Discover на 760 / 1280 / 1920 px;
- [ ] финальная полировка detail-страниц и пустых/ошибочных состояний на реальных данных;
- [ ] светлая тема и масштаб Windows 125 / 150 / 200%;
- [ ] keyboard-only и контрастный аудит;
- [ ] release-профилирование UI, artwork, audio и длинных коллекций;
- [x] финальный release build и non-demo startup smoke-test: процесс стабильно работает после 8 секунд запуска;
- [ ] ручная визуальная проверка без `--demo` на живом аккаунте (нативное окно недоступно текущему automation-сеансу).

Статус: этап 4 в работе — адаптивность, accessibility и живые состояния проходят финальную проверку<br>
Дата: 16 сентября 2026<br>
Референс: [zxcloli666/SoundCloud-Desktop](https://github.com/zxcloli666/SoundCloud-Desktop)

Текущий прогресс:

- [x] primitive → semantic → component tokens;
- [x] совместимый мост в существующий `Theme`;
- [x] палитры Dark/Light и пользовательский accent;
- [x] базовая геометрия command bar, player deck, context rail и поверхностей;
- [x] тесты контраста и тем;
- [x] responsive sidebar и новый AppShell;
- [x] command bar с Back/Forward, поиском и быстрым доступом к очереди;
- [x] context rail: docked на широком окне, overlay на узком;
- [x] новый player deck layout;
- [x] новый Home;
- [x] общие `Surface`, `ActionButton` и `SegmentedTabs`;
- [x] единые loading/empty/error-состояния и skeleton rows;
- [x] `Artwork`, `MediaCard`, `TrackRow` и `Waveform` собраны в общем слое;
- [x] keyboard focus rings и tooltips для кастомных контролов;
- [x] screenshot-тесты ключевых состояний компонентов.
- [x] Library с табами и Grid/List;
- [x] multi-select action bar в Library → Likes: Grid/List, Play/Queue/More/Clear и выбор всех по фильтру;
- [x] визуальная проверка компонента панели выделения Library: Dark/Light 960 px и Dark 420 px, три golden PNG;
- [ ] проверка панели выделения в полном окне Library: положение над плеером и совместная прокрутка;
- [x] Search с top result и keyboard navigation;
- [x] отдельный Discover с Vibe: маршрут, поле настроения, жанры, персональная волна и переход из Home;
- [ ] визуальная проверка Discover и Search;
- [x] Feed: пропуск невидимых карточек треков с кэшем измеренных высот;
- [x] Feed: первый проход с оценочной высотой неизвестных карточек, без полного построения ленты;
- [x] Feed: сброс измеренных высот при изменении ширины и масштаба;
- [x] playlist posts: виртуальная карусель с первого кадра, UI-тесты на 10 000 элементов и дальнюю прокрутку;
- [ ] визуальная проверка Feed: прокрутка, изменение ширины, фильтр репостов и уточнение оценочных высот (длина прокрутки может меняться по мере измерения);
- [x] изолированный release-замер памяти 10 000 Track: `docs/performance/2026-09-15.md`;
- [x] удаление копирования полной коллекции на каждый кадр в Likes Grid/List;
- [ ] полные замеры UI/artwork/audio и времени кадров на длинных коллекциях.

Golden PNG хранятся в `tests/snapshots/airwave`. Обновлять их нужно только после визуальной проверки: в PowerShell выполнить `$env:UPDATE_SNAPSHOTS = "true"`, затем `cargo test screenshot_tests`.

## Короткое решение

FastCloud не нужно переписывать на Next.js, React или Tauri. Референсный проект использует не Next.js, а **Tauri 2 + React 19 + Vite 7**. Это заметно легче Electron, но всё равно добавляет системный WebView, DOM, JavaScript-runtime и второй слой состояния между Rust и интерфейсом.

У FastCloud уже более подходящий фундамент для минимального расхода памяти: **Rust + eframe/egui без браузерного движка**, собственный аудиопайплайн, ограниченные кэши изображений и нативная интеграция. Рекомендуемое направление:

> Сохранить текущий нативный стек и сделать самостоятельный интерфейс **FastCloud Airwave**: спокойный графитовый фон, крупные обложки, волновая форма как фирменный мотив, компактная боковая навигация и цельный нижний плеер. Визуальная глубина создаётся слоями цвета и заранее рассчитанными градиентами, а не десятками дорогих blur-эффектов.

Это не копия SoundCloud Desktop. От референса берутся полезные идеи — единый accent, крупная иерархия, повторно используемые компоненты, виртуализация списков и прозрачная оболочка поверх пользовательских обоев. Стеклянные поверхности реализуются через семантические уровни альфа-прозрачности, а не дорогой blur на каждой карточке.

## 1. Что есть сейчас

### FastCloud

По текущему репозиторию:

- нативное приложение на Rust 2024, `eframe/egui 0.36`;
- собственное воспроизведение через `symphonia + cpal`, без встроенного браузера;
- полноценные Home, Feed, Library, Search, страницы трека, плейлиста и артиста;
- нижний плеер, очередь, эквалайзер, Winamp mini-player, системный трей, Discord Rich Presence;
- настраиваемая тема, accent, фон и его blur;
- RAM-кэш изображений уже ограничен: 16 MiB сжатых данных, около 48 MiB декодированных изображений и не более 64 готовых artwork;
- UI сейчас в основном повторяет обычный веб-интерфейс SoundCloud: верхняя навигация, плоские полки карточек, правая колонка и тонкая нижняя панель;
- два центральных файла слишком велики: `src/ui/mod.rs` и `src/ui/views.rs`. Редизайн без декомпозиции сделает их ещё сложнее.

### Референсный SoundCloud Desktop

В актуальном README заявлен стек Tauri 2, React 19, Vite 7, Tailwind CSS 4, Zustand, TanStack Query и Radix UI. В их `GLASS_UI_GUIDE.md` интерфейс строится вокруг прозрачных панелей, одного пользовательского accent, крупных hero-блоков, виртуальных списков и ограниченного количества blur-слоёв.

Полезно перенять:

- один accent-цвет вместо набора несвязанных цветов;
- hero-монолит для страниц трека, плейлиста и артиста;
- единые карточки, строки, кнопки и состояния;
- обязательную виртуализацию длинных коллекций;
- отключение лишних анимаций и подписок;
- пользовательский фон как часть атмосферы, а не как картинку под непрозрачной страницей.

Не стоит переносить:

- WebView и React только ради более удобной вёрстки;
- blur на каждой карточке;
- длинные декоративные анимации на всех экранах;
- слишком большие скругления и «стекло внутри стекла»;
- визуальную зависимость от конкретной картинки-фона.

## 2. Проблемы текущего интерфейса

По текущему Home-скриншоту и UI-коду основные проблемы такие:

1. **Нет собственного визуального характера.** Интерфейс близок к сайту SoundCloud, а FastCloud воспринимается как оболочка, хотя технически это гораздо более самостоятельный продукт.
2. **Слабая иерархия.** Почти все секции состоят из заголовка и одинаковых квадратных карточек. Неясно, что приложение предлагает включить прямо сейчас.
3. **Разорванная композиция.** Верхняя панель, центральная лента, правая колонка и нижний плеер выглядят как четыре независимых блока.
4. **Правая колонка перегружает Home.** Она постоянно отнимает ширину, хотя рекомендации, очередь и активность нужны в разных контекстах.
5. **Плеер слишком служебный.** Обложка и название находятся у правого края, а основной прогресс оторван от контекста трека.
6. **Фиксированные размеры плохо масштабируются.** В нижней панели используются жёсткие полосы шириной 220 и 352 px; при узком окне композиция быстро становится тесной.
7. **Дизайн-токены привязаны к SoundCloud.** Нынешние `Palette`, `Metrics` и типографика буквально повторяют токены сайта. Для нового визуального языка нужен собственный семантический слой.

## 3. Дизайн-концепция: FastCloud Airwave

### Образ

Airwave — это «музыка в воздухе»: тёмная спокойная среда, в которой обложка и waveform дают цвет, а интерфейс остаётся быстрым и собранным. Визуально это ближе к хорошему профессиональному аудиоплееру, чем к веб-сервису.

Ключевой фирменный элемент — **линия волны**:

- в логотипе и пустых состояниях;
- в hero текущего трека;
- в активной строке списка;
- в индикаторах загрузки;
- в мини-плеере.

Waveform не должен постоянно анимироваться. В покое это статичная форма; движение появляется только во время воспроизведения или прямого взаимодействия.

### Пять принципов

1. **Music first.** На каждом экране есть один очевидный главный объект и одно главное действие.
2. **Depth without blur.** Глубина создаётся контрастом поверхностей, границами, мягкими тенями и единственным цветным glow, а не постоянным размазыванием кадра.
3. **One accent.** SoundCloud-orange остаётся связью с источником музыки, но становится более тёплым фирменным `Pulse Orange`. Цвет из обложки допускается только в hero-фоне.
4. **Calm density.** Данные плотные, но не шумные: минимум рамок, метаданные появляются по контексту, действия — по hover/focus.
5. **Native speed is visible.** Мгновенные переходы, скелетоны без мерцания, стабильная геометрия и отсутствие скачущего layout должны ощущаться как часть дизайна.

## 4. Визуальная система

### Цвета

Основной режим — тёмный. Светлая тема поддерживается архитектурой токенов, но её полировка идёт после завершения основного редизайна.

| Токен | Тёмная тема | Назначение |
| --- | --- | --- |
| `canvas` | `#090A0D` | фон окна |
| `surface-1` | `#111319` | основная рабочая поверхность |
| `surface-2` | `#171A21` | карточки и панели |
| `surface-3` | `#1E222B` | hover, input, выбранные строки |
| `text-primary` | `#F5F7FA` | заголовки и основные значения |
| `text-secondary` | `#A0A8B5` | исполнитель, описание, время |
| `text-tertiary` | `#6F7783` | служебные подписи |
| `border-subtle` | `rgba(255,255,255,0.07)` | разделители и границы |
| `accent` | `#FF5B24` | главное действие и прогресс |
| `accent-hover` | `#FF7547` | hover/focus accent |
| `accent-soft` | `rgba(255,91,36,0.15)` | выбранные элементы |
| `success` | `#38C793` | успешные операции |
| `warning` | `#F6B94A` | предупреждения и rate limit |
| `danger` | `#F05A72` | ошибки и разрушительные действия |

Цвет от обложки рассчитывается один раз после загрузки изображения, затем:

- смешивается с `canvas` до контрастного тёмного оттенка;
- применяется только к hero-gradient и небольшому halo;
- не заменяет семантические цвета текста, ошибок и focus-ring;
- кэшируется по URL artwork, чтобы не пересчитываться каждый кадр.

### Иерархия токенов

Нужно уйти от прямого использования `Color32` и размеров внутри экранов.

```text
Primitives
  graphite.950, graphite.900, orange.500, space.4, radius.12
      ↓
Semantic
  canvas, surface.raised, text.muted, action.primary, focus.ring
      ↓
Component
  nav.item.active_bg, track.row.hover_bg, player.progress.fill
```

Названия описывают назначение, а не конкретный цвет. Это позволит менять тему без переписывания компонентов.

### Типографика

Оставить уже встроенный Inter и реальные начертания 400/500/700.

| Роль | Размер / высота | Вес | Использование |
| --- | --- | --- | --- |
| `display` | 40 / 44 | 700 | название трека или плейлиста в hero |
| `title-1` | 30 / 36 | 700 | заголовок страницы |
| `title-2` | 22 / 28 | 700 | секция |
| `title-3` | 16 / 22 | 600 | карточка, строка, диалог |
| `body` | 14 / 20 | 400 | основной текст |
| `label` | 13 / 16 | 600 | кнопка, вкладка, фильтр |
| `caption` | 12 / 16 | 400 | метаданные |
| `micro` | 10 / 14 | 600 | технический статус |

Не использовать uppercase для всех служебных заголовков: он остаётся только для коротких статусов вроде `PREVIEW`, `OFFLINE` и `GO+`.

### Геометрия

- базовый шаг: 4 px;
- основные отступы: 8, 12, 16, 24, 32, 48 px;
- кнопка: 36 px, primary-кнопка в hero: 44 px;
- строка трека: 56 px compact / 64 px comfortable;
- карточка: радиус 14 px;
- крупная панель: радиус 20 px;
- pill: полный радиус;
- touch/click target: минимум 32 × 32 px, для главных действий 40 × 40 px.

### Материалы и эффекты

Использовать три дешёвых материала:

1. `Flat` — непрозрачный `surface-1`, для длинных списков.
2. `Raised` — `surface-2`, тонкая верхняя подсветка и мягкая тень, для карточек.
3. `Ambient` — заранее рассчитанный градиент от artwork поверх `canvas`, только для hero.

Настоящий blur допускается только для пользовательского wallpaper и рассчитывается при загрузке/смене размера, а не каждый кадр. В режиме энергосбережения blur заменяется затемнённой уменьшенной копией.

### Иконки

Продолжить использовать уже включённые Lucide SVG. Правила:

- 16 px в строках;
- 18–20 px в навигации;
- 20–24 px в плеере;
- filled-форма только для `Play`, `Pause`, `Like active` и текущего пункта;
- никаких emoji в продуктовых контролах.

## 5. Новая оболочка приложения

### Широкое окно

```text
┌──────────────┬──────────────────────────────────────┬───────────────┐
│ FastCloud    │ ← →   Search music, people…    ⌘K  │ Context rail  │
│              ├──────────────────────────────────────┤ Queue / info  │
│ Home         │                                      │               │
│ Discover     │              Active view             │               │
│ Feed         │                                      │               │
│ Library      │                                      │               │
│              │                                      │               │
│ Playlists    │                                      │               │
│ Likes        │                                      │               │
│              │                                      │               │
│ Profile      │                                      │               │
├──────────────┴──────────────────────────────────────┴───────────────┤
│ artwork │ title / artist │ transport │ waveform + time │ volume … │
└─────────────────────────────────────────────────────────────────────┘
```

Размеры:

- command bar: 56 px;
- полная левая панель: 220 px;
- компактная левая панель: 72 px;
- контекстная правая панель: 320 px;
- player deck: 76 px;
- ширина контента: fluid, рекомендуемый максимум 1280 px без учёта боковых панелей.

### Адаптивность

| Ширина окна | Поведение |
| --- | --- |
| `≥ 1500` | полная навигация + закреплённая правая панель |
| `1100–1499` | навигация 72 px, правая панель до 300 px |
| `800–1099` | навигация 72 px, правая панель открывается drawer поверх контента |
| `< 800` | только иконки, упрощённый player deck, скрытые вторичные действия |

Минимальный поддерживаемый размер окна предлагается зафиксировать как 760 × 520 px. Это desktop-приложение; имитация мобильной вёрстки не нужна.

### Навигация

- `Home` — персональная стартовая страница;
- `Discover` — жанры, чарты и Vibe Search;
- `Feed` — свежие публикации подписок;
- `Library` — Likes, Playlists, Albums, Stations, History;
- пользовательские плейлисты показываются отдельным прокручиваемым блоком;
- `Settings` и mini-player находятся в нижней части панели;
- `Back/Forward` остаются в command bar и сохраняют текущую history-модель.

Активный пункт обозначается мягкой accent-подложкой и короткой waveform-риской, а не оранжевой полосой как на сайте SoundCloud.

### Command bar

- Back/Forward;
- глобальный поиск;
- сетевой статус показывается только при запросе дольше 1 секунды или rate limit;
- справа: кнопка очереди, mini-player, avatar/menu;
- поле поиска становится command palette по `Ctrl/Cmd+K`: недавние запросы, быстрые переходы и действия.

### Player deck

Нижний плеер становится главным постоянным элементом:

- слева: artwork 52 × 52, title, artist, Like;
- по центру: Previous, Play/Pause 40 px, Next, Shuffle, Repeat;
- под транспортом или рядом: waveform/progress с фиксированной геометрией;
- справа: Queue, device/status, volume, More;
- при ширине `< 1000 px` waveform сокращается до обычной progress-линии;
- обложка открывает Track Detail, имя — Artist Detail;
- drag по waveform даёт preview времени без запуска лишнего repaint после завершения drag.

## 6. Экраны

### Home — «что включить сейчас»

Первый экран не должен начинаться с ряда одинаковых Likes.

1. **Continue listening hero** — текущий или последний трек, большая обложка, waveform, Play/Resume и быстрый переход к очереди.
2. **Quick picks** — 6 компактных плиток: Likes, Daily Mix, Fresh from following, Vibe, History, Random station.
3. **Made for you** — одна горизонтальная полка рекомендаций.
4. **Fresh from artists** — релизы подписок с датой и типом релиза.
5. **Recently played** — компактный список, а не ещё один ряд крупных квадратов.

Правая панель по умолчанию показывает `Up next`, а рекомендации артистов открываются отдельным контекстом, когда очередь закрыта.

### Discover

- верхний Vibe Search: фраза естественным языком и последние mood-чипы;
- сетка жанров с простыми двухцветными градиентами без картинок;
- Trending и Charts — виртуализованный список;
- stations — карточки с круговой waveform-рамкой;
- один accent остаётся главным; цвета жанров используются только как локальные иллюстрации.

### Feed

- хронологическая лента;
- строка активности отделена от музыкального объекта;
- трек — горизонтальная карточка с artwork, короткой waveform и действиями;
- repost визуально легче обычной публикации;
- фильтр `All / Tracks / Reposts` закрепляется сверху при прокрутке.

### Library

- заголовок и segmented tabs: `Likes`, `Playlists`, `Albums`, `Stations`, `History`;
- переключатель `Grid / List` сохраняется отдельно для каждой коллекции;
- поиск внутри библиотеки и сортировка находятся в одной строке;
- длинные списки виртуализируются;
- multi-select показывает временную action bar над player deck.

### Search

- пустое состояние: недавние запросы, популярные жанры, подсказка про Vibe;
- результаты появляются после debounce и не меняют высоту вкладок;
- вкладки: `All`, `Tracks`, `People`, `Playlists`, `Albums`;
- `All` показывает один top result и короткие подборки остальных типов;
- keyboard navigation: Up/Down, Enter, Esc.

### Track / Playlist / Artist

Все detail-страницы используют единый `MediaHero`:

```text
┌──────────────────────────────────────────────────────────────┐
│ [artwork]  type / verified                                   │
│            Large title                         [stats]       │
│            artist · year · duration                           │
│            [Play] [Like] [Add to playlist] […]               │
└──────────────────────────────────────────────────────────────┘
```

- фон hero — статический тёмный градиент из доминирующего цвета artwork;
- playlist/album ниже использует таблицу треков;
- Artist: Popular, Releases, Albums, Playlists, Reposts;
- Track: большая waveform, описание, related tracks, comments только если они реально поддержаны API;
- sticky action bar появляется после ухода hero за верх экрана.

### Settings

Разделы в левой колонке, настройки справа:

- Appearance;
- Playback;
- Audio & Equalizer;
- Cache & Memory;
- Integrations;
- Shortcuts;
- Account;
- About.

В `Cache & Memory` добавить готовые профили:

- `Eco` — минимум памяти и фоновой активности;
- `Balanced` — рекомендуемый режим;
- `Quality` — больше artwork и предзагрузки.

Расширенные числовые лимиты можно оставить под раскрываемым `Advanced`, чтобы обычному пользователю не пришлось настраивать MiB вручную.

### Mini-player

Существующий Winamp mode остаётся как отдельная фича. Дополнительно нужен современный Airwave mini-player:

- 360 × 112 px;
- artwork, title/artist, Play/Pause, Next, Like;
- тонкая waveform внизу;
- без фоновой анимации и blur;
- переключение Winamp / Airwave в Settings.

## 7. Компоненты

Минимальный набор до переноса экранов:

| Компонент | Назначение |
| --- | --- |
| `AppShell` | responsive-раскладка и панели |
| `NavItem` | единое состояние default/hover/active/focus |
| `CommandSearch` | поиск и command palette |
| `Surface` | `Flat`, `Raised`, `Ambient` |
| `MediaCard` | Track, Playlist, Album с вариантами размера |
| `TrackRow` | компактная/обычная строка и selected/current states |
| `MediaHero` | Track, Playlist, Album, Artist |
| `Artwork` | размеры, placeholder, badge, загрузка |
| `Waveform` | static/current/interactive варианты |
| `IconButton` | размеры, tooltip, focus и active state |
| `ActionButton` | primary/secondary/quiet/danger |
| `SegmentedTabs` | Library/Search/Profile |
| `StatusChip` | Offline, Preview, Go+, Explicit |
| `EmptyState` | единый рисунок, текст и действие |
| `Skeleton` | стабильная геометрия без shimmer в Eco mode |
| `ContextRail` | Queue, track info, recommendations |
| `ToastCenter` | ограниченная очередь уведомлений |

Компоненты должны получать семантические варианты, а не произвольные цвета и отступы. Например, `ActionButton::primary`, а не `Button { fill: Color32, radius: 14 }` в каждом экране.

## 8. Решение по стеку

| Вариант | Плюсы | Минусы | Решение |
| --- | --- | --- | --- |
| Next.js + desktop shell | знакомая веб-разработка | Next не решает desktop-задачу, добавляет ненужную runtime/SSR-сложность | не использовать |
| Tauri + React/Vite | гибкая вёрстка, зрелые UI-библиотеки, легче Electron | WebView + JS + DOM, мост Rust↔UI, полный rewrite текущего egui | не использовать для основного клиента |
| Slint/Iced | декларативнее части egui-интерфейса | миграция всего UI, новые ограничения и риски | не сейчас |
| Текущий Rust + egui | минимальный runtime, существующая логика, единый язык, быстрый startup | дизайн требует ручных layout/widgets | **оставить** |

Next.js полезен для сайта проекта или web-preview, но не для основного desktop-клиента.

## 9. План архитектуры UI

Цель — не переписать всё за один большой PR, а создать новую систему рядом со старой и переносить экран за экраном.

```text
src/ui/
  design/
    mod.rs
    primitives.rs      # raw color/spacing/radius/type values
    tokens.rs          # semantic light/dark themes
    components.rs      # component-level tokens
    motion.rs          # duration/easing/reduced-motion
  layout/
    mod.rs
    shell.rs
    sidebar.rs
    command_bar.rs
    context_rail.rs
  components/
    mod.rs
    artwork.rs
    buttons.rs
    media_card.rs
    media_hero.rs
    segmented_tabs.rs
    track_row.rs
    waveform.rs
    states.rs
  views/
    mod.rs
    home.rs
    discover.rs
    feed.rs
    library.rs
    search.rs
    track.rs
    playlist.rs
    artist.rs
    settings.rs
  player_deck.rs
```

Переезд выполняется постепенно:

- старые `Theme`, `Metrics`, `widgets` получают compatibility-обёртки;
- сначала новый shell использует старые view-функции;
- затем экраны переносятся по одному;
- только после последнего переноса удаляются старые токены и helpers;
- бизнес-логика `Player`, `Store`, API, auth, cache и desktop integrations не переписывается.

## 10. RAM, GPU и производительность

### Целевые бюджеты

Это не обещание до замера, а gate для реализации. Перед первым UI-изменением нужно записать baseline на одинаковом наборе данных.

| Сценарий | Предварительная цель Balanced |
| --- | --- |
| Home через 60 секунд после запуска | ≤ 90 MiB working set на Windows |
| стабильное воспроизведение 30 минут | ≤ 130 MiB, без устойчивого роста |
| переключение 20 detail-страниц | возврат к пределах +10 MiB от settled baseline |
| artwork CPU budget | 32 MiB decoded + 8 MiB compressed |
| количество готовых artwork | 48 Balanced / 32 Eco |
| frame time при взаимодействии | p95 ≤ 16.7 ms на целевом слабом устройстве |
| idle CPU | < 1% после завершения загрузок |
| скрытое/свёрнутое окно | отсутствие постоянного 30/60 FPS repaint |

Для Linux/macOS абсолютные значения нужно хранить отдельно: системные Web/GL/Window подсистемы считают память по-разному. Главный CI-gate — отсутствие регрессии относительно baseline на той же платформе.

### Что изменить

1. **Размер artwork по месту использования.** Сейчас CDN URL повышается до 500 × 500 даже там, где карточке достаточно 160–200 px. Добавить варианты `Thumbnail`, `Card`, `Hero` и отдельные cache keys.
2. **Снизить default image budget после измерения.** Начать с 32 MiB decoded, 8 MiB compressed, 48 ready; оставить текущие 48/16/64 для профиля Quality.
3. **Не держать CPU и GPU копии бесконечно.** При eviction продолжать вызывать `ctx.forget_image`, а также проверять, что variant URL не создаёт дубликаты одного размера.
4. **Wallpaper downsample.** Декодировать исходник, сразу уменьшать до размера viewport с небольшим запасом, blur выполнять один раз, старую texture освобождать после успешной замены.
5. **Виртуализировать строки.** Для Feed, Likes, History и больших playlist использовать `ScrollArea::show_rows`, а не создавать все widgets.
6. **Ограничить Store.** Держать только несколько последних страниц каждой пагинации или LRU по route key; не копить все открытые профили до конца сессии.
7. **Waveform cache.** Хранить нормализованные samples, а не готовые большие изображения; ограничить число записей и переиспользовать resampled buffers для типовых ширин.
8. **Repaint по событию.** 60 FPS только во время drag/active visualizer. Для playback time достаточно 4–10 FPS; idle — только события и редкие таймеры.
9. **Остановка невидимых эффектов.** Visualizer, spinner, hover transition и waveform progress не обновляются при hidden/minimized и вне viewport.
10. **Без blur в карточках.** Никаких дополнительных off-screen render targets для каждой плитки.
11. **Bounded concurrency.** Ограничить одновременную загрузку и decode artwork; очередь должна отдавать приоритет видимому viewport.
12. **Не клонировать большие коллекции для каждого кадра.** Проверить hot paths в `views.rs`; использовать `Arc`, срезы и локальные идентификаторы там, где borrow checker позволяет.

### Профили памяти

| Параметр | Eco | Balanced | Quality |
| --- | ---: | ---: | ---: |
| compressed artwork | 4 MiB | 8 MiB | 16 MiB |
| decoded artwork | 24 MiB | 32 MiB | 48 MiB |
| ready artwork count | 32 | 48 | 64 |
| card CDN size | 160 px | 200 px | 320 px |
| hero CDN size | 320 px | 500 px | 500 px |
| playback UI refresh | 4 FPS | 8 FPS | 15 FPS |
| visualizer | off по умолчанию | 30 FPS | 30/60 FPS на выбор |
| prefetch | следующий трек | 1–2 трека | 2–3 трека |

Числа нужно подтвердить профилировкой. Если уменьшение decoded budget вызывает постоянный decode/evict churn, Balanced возвращается к 40–48 MiB: меньшая цифра не должна ухудшать отзывчивость и расход CPU.

### Как измерять

- Windows: `Get-Process fastcloud` (`WorkingSet64`, `PrivateMemorySize64`, CPU time) после scripted warm-up;
- Linux: `/usr/bin/time -v`, `smem`, при необходимости heaptrack;
- macOS: `time -l`, Instruments Allocations;
- Rust heap: DHAT или jemalloc profiling в отдельной feature-сборке;
- GPU: RenderDoc/PIX только для подозрительных texture spikes;
- сценарии: cold start, warm Home, scroll 500 элементов, 20 переходов detail, 30 минут playback, wallpaper 4K, mini-player.

Результаты хранить в `docs/performance/YYYY-MM-DD.md` с commit SHA, OS, разрешением окна и режимом памяти.

## 11. Этапы реализации

### Этап 0 — baseline и защита от регрессий (1–2 дня)

- собрать release с текущим кодом;
- записать RAM/CPU/startup baseline для Windows и минимум одной второй ОС;
- сделать эталонные скриншоты всех маршрутов в `--demo`;
- добавить debug overlay с FPS, repaint reason, artwork entries/bytes и Store counts;
- зафиксировать минимальный размер окна и тестовые разрешения.

Готово, когда любой следующий этап можно сравнить цифрами и скриншотами.

### Этап 1 — дизайн-токены и новый shell (2–4 дня)

- добавить primitive → semantic → component tokens;
- сохранить Dark/Light/System и пользовательский accent;
- внедрить `AppShell`, responsive sidebar, command bar и context rail;
- временно отрисовать внутри старые views;
- добавить reduced motion и focus-ring.

Готово, когда все старые маршруты открываются в новой оболочке без изменения бизнес-логики.

### Этап 2 — фундаментальные компоненты (3–5 дней)

- `Surface`, `Artwork`, `IconButton`, `ActionButton`;
- `MediaCard`, `TrackRow`, `SegmentedTabs`;
- `Waveform`, skeleton и empty/error states;
- keyboard/focus/tooltip состояния;
- screenshot tests ключевых вариантов.

Готово, когда экран можно собрать без raw colors, случайных размеров и дублирования painting-кода.

### Этап 3 — player deck и Home (4–6 дней)

- заменить нижнюю панель адаптивным player deck;
- собрать Continue Listening hero и Quick Picks;
- преобразовать Recently Played в компактный список;
- подключить контекстную очередь;
- проверить playback, seeking, volume, shuffle, repeat и preview fallback.

Это первый релизный вертикальный срез и лучший момент для пользовательского теста.

### Этап 4 — Discover, Feed, Library, Search (6–9 дней)

- отдельный Discover с Vibe;
- новая виртуализованная Feed;
- Library с табами, Grid/List и multi-select action bar;
- Search с top result и keyboard navigation;
- измерение памяти на длинных коллекциях.

### Этап 5 — detail-страницы (5–7 дней)

- общий `MediaHero`;
- Track, Playlist/Album, Artist;
- artwork-derived ambient gradient;
- sticky actions и контекстная правая панель;
- проверка отсутствия texture leak при переходах.

### Этап 6 — Settings, mini-player и темы (3–5 дней)

- новая структура Settings;
- Eco/Balanced/Quality;
- Airwave mini-player без удаления Winamp mode;
- полировка светлой темы;
- contrast и reduced-motion аудит.

### Этап 7 — hardening и выпуск (3–5 дней)

- сравнить все performance-сценарии с baseline;
- проверить Windows/Linux/macOS и scaling 100/125/150/200%;
- keyboard-only проход;
- screen-reader labels там, где их поддерживает egui/accesskit;
- обновить README screenshots;
- beta feature flag на один релиз, затем сделать Airwave default.

Ориентир для одного разработчика: **4–6 недель**, если API и playback не переписываются. Первый законченный Home + player deck можно получить примерно за 1.5–2 недели.

## 12. Порядок PR

Чтобы изменения оставались проверяемыми:

1. `ui: add Airwave tokens and compatibility theme`;
2. `ui: add responsive app shell`;
3. `ui: add shared media components`;
4. `ui: replace player bar with player deck`;
5. `ui: redesign home`;
6. `perf: add artwork variants and memory profiles`;
7. `ui: redesign discover feed library search`;
8. `ui: add shared media hero and migrate details`;
9. `ui: redesign settings and add Airwave mini-player`;
10. `perf: benchmark and enforce release budgets`.

Не смешивать массовое перемещение файлов, визуальные изменения и cache-оптимизацию в одном PR: иначе невозможно понять источник регрессии.

## 13. Definition of Done

Редизайн считается законченным, когда:

- FastCloud визуально узнаваем без логотипа;
- все основные маршруты используют новый shell и компоненты;
- ни один production view не использует raw hex/случайный spacing вне token-файлов;
- интерфейс корректен от 760 × 520 до 4K и при 100–200% scale;
- все основные действия доступны клавиатурой;
- contrast текста и контролов соответствует WCAG AA там, где применимо;
- reduced motion действительно останавливает необязательное движение;
- 500+ строк не создаются одновременно вне viewport;
- после 30 минут playback и серии переходов память стабилизируется;
- Balanced не превышает согласованный performance budget либо отклонение задокументировано;
- Winamp mode, системные медиа-кнопки, tray, Discord, auth и CLI не сломаны;
- README содержит новые скриншоты и описание режимов памяти.

## 14. Что намеренно не входит в редизайн

- переписывание API, auth и аудиодекодера;
- серверный backend;
- Next.js/React-версия desktop-клиента;
- копирование экранов или CSS из референсного проекта;
- постоянные частицы, 3D, shader backgrounds и blur на каждой карточке;
- удаление Winamp mode;
- мобильное приложение.

## 15. Источники и точки в текущем коде

Внешние:

- [SoundCloud-Desktop: README и стек](https://github.com/zxcloli666/SoundCloud-Desktop#стек)
- [SoundCloud-Desktop: Glass UI / Vision Pro Cookbook](https://github.com/zxcloli666/SoundCloud-Desktop/blob/main/desktop/GLASS_UI_GUIDE.md)
- [SoundCloud-Desktop: desktop/src](https://github.com/zxcloli666/SoundCloud-Desktop/tree/main/desktop/src)

FastCloud:

- [`Cargo.toml`](../Cargo.toml) — текущий нативный стек;
- [`src/ui/theme.rs`](../src/ui/theme.rs) — существующие SoundCloud-derived токены;
- [`src/ui/mod.rs`](../src/ui/mod.rs) — shell и состояние приложения;
- [`src/ui/views.rs`](../src/ui/views.rs) — текущие экраны;
- [`src/ui/player_bar.rs`](../src/ui/player_bar.rs) — нижний плеер;
- [`src/images.rs`](../src/images.rs) — RAM/GPU budget artwork и wallpaper;
- [`docs/screenshots/home.png`](screenshots/home.png) — текущий Home.

## 16. Первый практический шаг

Начать не с полного редизайна всех страниц, а с одного вертикального среза:

1. baseline памяти и скриншоты;
2. новые токены;
3. shell;
4. player deck;
5. Home;
6. повторный замер.

Если этот срез выглядит современно и укладывается в RAM/CPU budget, система масштабируется на остальные экраны. Если нет — можно скорректировать визуальный язык, не переписывая весь клиент.
