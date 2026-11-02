//! The annotation panels of the inspector (mockup 3c and the main window): the common part, the
//! key strip and the sections of the kinds.

pub const DUPLICATE: &str = "Дублировать";

/// «Ключ 2 из 3 · 0:33.40»: the playhead stands on a key of the annotation.
pub fn key_of(number: usize, count: usize, at: &str) -> String {
    format!("Ключ {number} из {count} · {at}")
}

/// «Ключа нет · всего 3»: the annotation has keys, none at the playhead.
pub fn no_key(count: usize) -> String {
    format!("Ключа нет · всего {count}")
}

pub const NO_KEYS: &str = "Ключей нет";
pub const KEY_PREVIOUS: &str = "Предыдущий ключ";
pub const KEY_NEXT: &str = "Следующий ключ";
pub const KEY_ON_NOTE: &str = "Правка меняет этот ключ.";
pub const KEY_OFF_NOTE: &str =
    "Указатель не на ключе: правка меняет значение во всей аннотации и во всех ключах.";

pub const MOTION: &str = "Вход и выход";
pub const ENTER: &str = "Вход";
pub const EXIT: &str = "Выход";
pub const ENTER_TIME: &str = "Длина входа";
pub const EXIT_TIME: &str = "Длина выхода";
pub const ANIM_NONE: &str = "Нет";
pub const ANIM_FADE: &str = "Растворение";
pub const ANIM_POP: &str = "Появление";
pub const ANIM_SLIDE_LEFT: &str = "Выезд слева";
pub const ANIM_SLIDE_RIGHT: &str = "Выезд справа";
pub const ANIM_SLIDE_TOP: &str = "Выезд сверху";
pub const ANIM_SLIDE_BOTTOM: &str = "Выезд снизу";
pub const ANIM_DRAW: &str = "Прорисовка";
pub const ANIM_DRAW_ONLY: &str = "Прорисовка · фигура, рисунок";
pub const ANIM_TYPE: &str = "Печать";
pub const ANIM_TYPE_ONLY: &str = "Печать · только текст";

pub const X: &str = "X";
pub const Y: &str = "Y";
pub const WIDTH: &str = "Ширина";
pub const HEIGHT: &str = "Высота";
pub const ROTATION: &str = "Поворот";
pub const DEGREES: &str = "°";
pub const TIME: &str = "Время";
pub const START: &str = "Начало";
pub const END: &str = "Конец";
pub const TRACK: &str = "Дорожка";
pub const LAYER: &str = "Слой";
pub const LAYER_TOP: &str = "Наверх";
pub const LAYER_UP: &str = "Выше";
pub const LAYER_DOWN: &str = "Ниже";
pub const LAYER_BOTTOM: &str = "Вниз";
pub const PLACE_NOTE: &str = "Место и размер — в пикселях кадра.";

pub const TEXT_HINT: &str = "Текст аннотации";
pub const TEXT_NOTE: &str = "Enter — новая строка, клик мимо применяет, Esc отменяет.";
pub const FONT: &str = "Шрифт";
pub const SIZE: &str = "Размер";
pub const STYLE: &str = "Начертание";
pub const BOLD: &str = "Жирный";
pub const ITALIC: &str = "Курсив";
pub const UNDERLINE: &str = "Подчёркнутый";
pub const ALIGN: &str = "Выравнивание";
pub const LEFT: &str = "Слева";
pub const CENTER: &str = "Центр";
pub const RIGHT: &str = "Справа";
pub const COLORS: &str = "Цвет · фон";
pub const COLOR: &str = "Цвет букв";
pub const PLATE: &str = "Цвет плашки";
pub const PLATE_NOTE: &str = "Прозрачность фона — в его цвете: 0 % убирает плашку.";
pub const RADIUS: &str = "Скругление";
pub const PX: &str = " px";

pub const ARROW: &str = "Стрелка";
pub const RECTANGLE: &str = "Прямоуг.";
pub const ELLIPSE: &str = "Эллипс";
pub const CIRCLE: &str = "Круг";
pub const LINE: &str = "Линия";
pub const HIGHLIGHT: &str = "Подсв.";
pub const STROKE: &str = "Цвет линии";
pub const FILL: &str = "Заливка";
pub const SHAPE_COLORS: &str = "Линия · заливка";
pub const THICKNESS: &str = "Толщина";
pub const OUTLINE: &str = "Обводка";
pub const GRADIENT: &str = "Градиент";
pub const OPACITY: &str = "Прозрачность";
pub const GLOW: &str = "Свечение";
pub const ANGLE: &str = "Направление";
pub const CYCLE: &str = "Круг градиента";
pub const CYCLE_NOTE: &str = "0 с — градиент стоит.";
pub const TAIL_X: &str = "Начало X";
pub const TAIL_Y: &str = "Начало Y";
pub const TIP_X: &str = "Конец X";
pub const TIP_Y: &str = "Конец Y";
pub const ENDS_NOTE: &str = "Концы — в процентах рамки аннотации.";

pub const FILE: &str = "Файл";
pub const NO_FILE: &str = "нет картинки";
pub const REPLACE: &str = "Заменить…";
pub const PICK_PICTURE: &str = "Картинка для аннотации";
pub const PICTURES: &str = "Картинки";
pub const PICTURE_NOTE: &str =
    "PNG, JPEG, WebP, GIF (анимация играет). Копируется в проект; картинка вписывается в рамку.";

/// «схема.png · 240 × 180»: the picture's file and size in px.
pub fn picture(name: &str, w: u32, h: u32) -> String {
    format!("{name} · {w} × {h}")
}

pub const PENCIL: &str = "Цвет карандаша";
pub const DRAWING_COLOR: &str = "Цвет";
pub const CLEAR: &str = "Очистить рисунок";
pub const DRAWING_NOTE: &str = "Цвет и толщина меняются у всех штрихов карандаша. На превью: ЛКМ — карандаш, Shift+ЛКМ — ластик, Alt+ЛКМ — прямая, ПКМ — двигать.";

/// «Штрихов: 3»: how many strokes the drawing has.
pub fn strokes(count: usize) -> String {
    format!("Штрихов: {count}")
}

pub const STRENGTH: &str = "Сила";
pub const TINT: &str = "Тонировка";
pub const TINT_COLOR: &str = "Цвет тонировки";
pub const TINT_COVER: &str = "Сила тонировки";
pub const TINT_NOTE: &str = "100 % — сплошная заливка («цензура»).";
