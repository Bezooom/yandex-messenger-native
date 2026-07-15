use gtk::prelude::*;
use gtk::{
    Box as GtkBox, Button, FlowBox, FlowBoxChild, Label, Orientation, ScrolledWindow, Widget,
};
use std::cell::RefCell;
use std::rc::Rc;

/// One emoji category tab.
struct EmojiCategory {
    title: &'static str,
    icon: &'static str,
    emojis: &'static [&'static str],
}

const CATEGORIES: &[EmojiCategory] = &[
    EmojiCategory {
        title: "Смайлы",
        icon: "😀",
        emojis: &[
            "😀",
            "😃",
            "😄",
            "😁",
            "😆",
            "😅",
            "🤣",
            "😂",
            "🙂",
            "🙃",
            "😉",
            "😊",
            "😇",
            "🥰",
            "😍",
            "🤩",
            "😘",
            "😗",
            "☺️",
            "😚",
            "😙",
            "🥲",
            "😋",
            "😛",
            "😜",
            "🤪",
            "😝",
            "🤑",
            "🤗",
            "🤭",
            "🤫",
            "🤔",
            "🤐",
            "🤨",
            "😐",
            "😑",
            "😶",
            "😏",
            "😒",
            "🙄",
            "😬",
            "😮‍💨",
            "🤥",
            "😌",
            "😔",
            "😪",
            "🤤",
            "😴",
            "😷",
            "🤒",
            "🤕",
            "🤢",
            "🤮",
            "🤧",
            "🥵",
            "🥶",
            "🥴",
            "😵",
            "🤯",
            "🤠",
            "🥳",
            "🥸",
            "😎",
            "🤓",
            "🧐",
            "😕",
            "😟",
            "🙁",
            "☹️",
            "😮",
            "😯",
            "😲",
            "😳",
            "🥺",
            "😦",
            "😧",
            "😨",
            "😰",
            "😥",
            "😢",
            "😭",
            "😱",
            "😖",
            "😣",
            "😞",
            "😓",
            "😩",
            "😫",
            "🥱",
            "😤",
            "😡",
            "😠",
            "🤬",
            "😈",
            "👿",
            "💀",
            "☠️",
            "💩",
            "🤡",
            "👹",
            "👺",
            "👻",
            "👽",
            "👾",
            "🤖",
            "😺",
            "😸",
            "😹",
            "😻",
            "😼",
            "😽",
            "🙀",
            "😿",
            "😾",
        ],
    },
    EmojiCategory {
        title: "Жесты",
        icon: "👍",
        emojis: &[
            "👋", "🤚", "🖐️", "✋", "🖖", "👌", "🤌", "🤏", "✌️", "🤞", "🤟", "🤘", "🤙", "👈",
            "👉", "👆", "🖕", "👇", "☝️", "👍", "👎", "✊", "👊", "🤛", "🤜", "👏", "🙌", "👐",
            "🤲", "🤝", "🙏", "✍️", "💅", "🤳", "💪", "🦾", "🦿", "🦵", "🦶", "👂", "🦻", "👃",
            "🧠", "🫀", "🫁", "🦷", "🦴", "👀", "👁️", "👅", "👄", "💋", "🩸",
        ],
    },
    EmojiCategory {
        title: "Сердца",
        icon: "❤️",
        emojis: &[
            "❤️", "🧡", "💛", "💚", "💙", "💜", "🖤", "🤍", "🤎", "💔", "❣️", "💕", "💞", "💓",
            "💗", "💖", "💘", "💝", "💟", "☮️", "✝️", "☪️", "🕉️", "☸️", "✡️", "🔯", "🕎", "☯️",
            "☦️", "🛐", "⛎", "♈", "♉", "♊", "♋", "♌", "♍", "♎", "♏", "♐", "♑", "♒",
            "♓", "🆔", "⚛️", "🉑", "☢️", "☣️",
        ],
    },
    EmojiCategory {
        title: "Животные",
        icon: "🐱",
        emojis: &[
            "🐶",
            "🐱",
            "🐭",
            "🐹",
            "🐰",
            "🦊",
            "🐻",
            "🐼",
            "🐻‍❄️",
            "🐨",
            "🐯",
            "🦁",
            "🐮",
            "🐷",
            "🐽",
            "🐸",
            "🐵",
            "🙈",
            "🙉",
            "🙊",
            "🐒",
            "🐔",
            "🐧",
            "🐦",
            "🐤",
            "🐣",
            "🐥",
            "🦆",
            "🦅",
            "🦉",
            "🦇",
            "🐺",
            "🐗",
            "🐴",
            "🦄",
            "🐝",
            "🪱",
            "🐛",
            "🦋",
            "🐌",
            "🐞",
            "🐜",
            "🪰",
            "🪲",
            "🪳",
            "🦟",
            "🦗",
            "🕷️",
            "🕸️",
            "🦂",
            "🐢",
            "🐍",
            "🦎",
            "🦖",
            "🦕",
            "🐙",
            "🦑",
            "🦐",
            "🦞",
            "🦀",
            "🐡",
            "🐠",
            "🐟",
            "🐬",
            "🐳",
            "🐋",
            "🦈",
            "🐊",
            "🐅",
            "🐆",
            "🦓",
            "🦍",
            "🦧",
            "🦣",
            "🐘",
            "🦛",
            "🦏",
            "🐪",
            "🐫",
            "🦒",
            "🦘",
            "🦬",
            "🐃",
            "🐂",
            "🐄",
            "🐎",
            "🐖",
            "🐏",
            "🐑",
            "🦙",
            "🐐",
            "🦌",
            "🐕",
            "🐩",
            "🦮",
            "🐕‍🦺",
            "🐈",
            "🐈‍⬛",
            "🪶",
            "🐓",
            "🦃",
            "🦤",
            "🦚",
            "🦜",
            "🦢",
            "🦩",
            "🕊️",
            "🐇",
            "🦝",
            "🦨",
            "🦡",
            "🦫",
            "🦦",
            "🦥",
            "🐁",
            "🐀",
            "🐿️",
            "🦔",
        ],
    },
    EmojiCategory {
        title: "Еда",
        icon: "🍕",
        emojis: &[
            "🍏", "🍎", "🍐", "🍊", "🍋", "🍌", "🍉", "🍇", "🍓", "🫐", "🍈", "🍒", "🍑", "🥭",
            "🍍", "🥥", "🥝", "🍅", "🍆", "🥑", "🥦", "🥬", "🥒", "🌶️", "🫑", "🌽", "🥕", "🫒",
            "🧄", "🧅", "🥔", "🍠", "🥐", "🥯", "🍞", "🥖", "🥨", "🧀", "🥚", "🍳", "🧈", "🥞",
            "🧇", "🥓", "🥩", "🍗", "🍖", "🦴", "🌭", "🍔", "🍟", "🍕", "🫓", "🥪", "🥙", "🧆",
            "🌮", "🌯", "🫔", "🥗", "🥘", "🫕", "🥫", "🍝", "🍜", "🍲", "🍛", "🍣", "🍱", "🥟",
            "🦪", "🍤", "🍙", "🍚", "🍘", "🍥", "🥠", "🥮", "🍢", "🍡", "🍧", "🍨", "🍦", "🥧",
            "🧁", "🍰", "🎂", "🍮", "🍭", "🍬", "🍫", "🍿", "🍩", "🍪", "🌰", "🥜", "🍯", "🥛",
            "🍼", "☕", "🫖", "🍵", "🧃", "🥤", "🧋", "🍶", "🍺", "🍻", "🥂", "🍷", "🥃", "🍸",
            "🍹", "🧉", "🍾", "🧊",
        ],
    },
    EmojiCategory {
        title: "Путешествия",
        icon: "✈️",
        emojis: &[
            "🚗", "🚕", "🚙", "🚌", "🚎", "🏎️", "🚓", "🚑", "🚒", "🚐", "🛻", "🚚", "🚛", "🚜",
            "🦯", "🦽", "🦼", "🛴", "🚲", "🛵", "🏍️", "🛺", "🚨", "🚔", "🚍", "🚘", "🚖", "🚡",
            "🚠", "🚟", "🚃", "🚋", "🚞", "🚝", "🚄", "🚅", "🚈", "🚂", "🚆", "🚇", "🚊", "🚉",
            "✈️", "🛫", "🛬", "🛩️", "💺", "🛰️", "🚀", "🛸", "🚁", "🛶", "⛵", "🚤", "🛥️", "🛳️",
            "⛴️", "🚢", "⚓", "🪝", "⛽", "🚧", "🚦", "🚥", "🚏", "🗺️", "🗿", "🗽", "🗼", "🏰",
            "🏯", "🏟️", "🎡", "🎢", "🎠", "⛲", "⛱️", "🏖️", "🏝️", "🏜️", "🌋", "⛰️", "🏔️", "🗻",
            "🏕️", "⛺", "🏠", "🏡", "🏘️", "🏚️", "🏗️", "🏭", "🏢", "🏬", "🏣", "🏤", "🏥", "🏦",
            "🏨", "🏪", "🏫", "🏩", "💒", "🏛️", "⛪", "🕌", "🕍", "🛕", "🕋",
        ],
    },
    EmojiCategory {
        title: "Объекты",
        icon: "💡",
        emojis: &[
            "⌚", "📱", "📲", "💻", "⌨️", "🖥️", "🖨️", "🖱️", "🖲️", "🕹️", "🗜️", "💽", "💾", "💿",
            "📀", "📼", "📷", "📸", "📹", "🎥", "📽️", "🎞️", "📞", "☎️", "📟", "📠", "📺", "📻",
            "🎙️", "🎚️", "🎛️", "🧭", "⏱️", "⏲️", "⏰", "🕰️", "⌛", "⏳", "📡", "🔋", "🔌", "💡",
            "🔦", "🕯️", "🪔", "🧯", "🛢️", "💸", "💵", "💴", "💶", "💷", "🪙", "💰", "💳", "💎",
            "⚖️", "🪜", "🧰", "🪛", "🔧", "🔨", "⚒️", "🛠️", "⛏️", "🪚", "🔩", "⚙️", "🪤", "🧱",
            "⛓️", "🧲", "🔫", "💣", "🧨", "🪓", "🔪", "🗡️", "⚔️", "🛡️", "🚬", "⚰️", "🪦", "⚱️",
            "🏺", "🔮", "📿", "🧿", "💈", "⚗️", "🔭", "🔬", "🕳️", "🩹", "🩺", "💊", "💉", "🩸",
            "🧬", "🦠", "🧫", "🧪", "🌡️", "🧹", "🪠", "🧺", "🧻", "🚽", "🚰", "🚿", "🛁", "🛀",
            "🧼", "🪥", "🪒", "🧽", "🪣", "🧴", "🛎️", "🔑", "🗝️", "🚪", "🪑", "🛋️", "🛏️", "🛌",
            "🧸", "🪆", "🖼️", "🪞", "🪟", "🛍️", "🛒", "🎁", "🎈", "🎏", "🎀", "🪄", "🪅", "🎊",
            "🎉", "🎎", "🏮", "🎐", "🧧", "✉️", "📩", "📨", "📧", "💌", "📥", "📤", "📦", "🏷️",
            "🪧", "📪", "📫", "📬", "📭", "📮", "📯", "📜", "📃", "📄", "📑", "🧾", "📊", "📈",
            "📉", "🗒️", "🗓️", "📆", "📅", "🗑️", "📇", "🗃️", "🗳️", "🗄️", "📋", "📁", "📂", "🗂️",
            "🗞️", "📰", "📓", "📔", "📒", "📕", "📗", "📘", "📙", "📚", "📖", "🔖", "🧷", "🔗",
            "📎", "🖇️", "📐", "📏", "🧮", "📌", "📍", "✂️", "🖊️", "🖋️", "✒️", "🖌️", "🖍️", "📝",
            "✏️", "🔍", "🔎", "🔏", "🔐", "🔒", "🔓",
        ],
    },
    EmojiCategory {
        title: "Символы",
        icon: "🔥",
        emojis: &[
            "❤️",
            "💛",
            "💚",
            "💙",
            "💜",
            "🖤",
            "🤍",
            "🤎",
            "💔",
            "❣️",
            "💕",
            "💞",
            "💓",
            "💗",
            "💖",
            "💘",
            "💝",
            "💟",
            "☮️",
            "✝️",
            "☪️",
            "🕉️",
            "☸️",
            "✡️",
            "🔯",
            "🕎",
            "☯️",
            "☦️",
            "🛐",
            "⛎",
            "♈",
            "♉",
            "♊",
            "♋",
            "♌",
            "♍",
            "♎",
            "♏",
            "♐",
            "♑",
            "♒",
            "♓",
            "🆔",
            "⚛️",
            "🉑",
            "☢️",
            "☣️",
            "📴",
            "📳",
            "🈶",
            "🈚",
            "🈸",
            "🈺",
            "🈷️",
            "✴️",
            "🆚",
            "💮",
            "🉐",
            "㊙️",
            "㊗️",
            "🈴",
            "🈵",
            "🈹",
            "🈲",
            "🅰️",
            "🅱️",
            "🆎",
            "🆑",
            "🅾️",
            "🆘",
            "❌",
            "⭕",
            "🛑",
            "⛔",
            "📛",
            "🚫",
            "💯",
            "💢",
            "♨️",
            "🚷",
            "🚯",
            "🚳",
            "🚱",
            "🔞",
            "📵",
            "🚭",
            "❗",
            "❕",
            "❓",
            "❔",
            "‼️",
            "⁉️",
            "🔅",
            "🔆",
            "〽️",
            "⚠️",
            "🚸",
            "🔱",
            "⚜️",
            "🔰",
            "♻️",
            "✅",
            "🈯",
            "💹",
            "❇️",
            "✳️",
            "❎",
            "🌐",
            "💠",
            "Ⓜ️",
            "🌀",
            "💤",
            "🏧",
            "🚾",
            "♿",
            "🅿️",
            "🛗",
            "🈳",
            "🈂️",
            "🛂",
            "🛃",
            "🛄",
            "🛅",
            "🚹",
            "🚺",
            "🚼",
            "⚧️",
            "🚻",
            "🚮",
            "🎦",
            "📶",
            "🈁",
            "🔣",
            "ℹ️",
            "🔤",
            "🔡",
            "🔠",
            "🆖",
            "🆗",
            "🆙",
            "🆒",
            "🆕",
            "🆓",
            "0️⃣",
            "1️⃣",
            "2️⃣",
            "3️⃣",
            "4️⃣",
            "5️⃣",
            "6️⃣",
            "7️⃣",
            "8️⃣",
            "9️⃣",
            "🔟",
            "🔢",
            "#️⃣",
            "*️⃣",
            "⏏️",
            "▶️",
            "⏸️",
            "⏯️",
            "⏹️",
            "⏺️",
            "⏭️",
            "⏮️",
            "⏩",
            "⏪",
            "⏫",
            "⏬",
            "◀️",
            "🔼",
            "🔽",
            "➡️",
            "⬅️",
            "⬆️",
            "⬇️",
            "↗️",
            "↘️",
            "↙️",
            "↖️",
            "↕️",
            "↔️",
            "↪️",
            "↩️",
            "⤴️",
            "⤵️",
            "🔀",
            "🔁",
            "🔂",
            "🔄",
            "🔃",
            "🎵",
            "🎶",
            "➕",
            "➖",
            "➗",
            "✖️",
            "♾️",
            "💲",
            "💱",
            "™️",
            "©️",
            "®️",
            "👁️‍🗨️",
            "🔚",
            "🔙",
            "🔛",
            "🔝",
            "🔜",
            "〰️",
            "➰",
            "➿",
            "✔️",
            "☑️",
            "🔘",
            "🔴",
            "🟠",
            "🟡",
            "🟢",
            "🔵",
            "🟣",
            "⚫",
            "⚪",
            "🟤",
            "🔺",
            "🔻",
            "🔸",
            "🔹",
            "🔶",
            "🔷",
            "🔳",
            "🔲",
            "▪️",
            "▫️",
            "◾",
            "◽",
            "◼️",
            "◻️",
            "🟥",
            "🟧",
            "🟨",
            "🟩",
            "🟦",
            "🟪",
            "⬛",
            "⬜",
            "🟫",
            "🔈",
            "🔇",
            "🔉",
            "🔊",
            "🔔",
            "🔕",
            "📣",
            "📢",
            "💬",
            "💭",
            "🗯️",
            "♠️",
            "♣️",
            "♥️",
            "♦️",
            "🃏",
            "🎴",
            "🀄",
            "🕐",
            "🕑",
            "🕒",
            "🕓",
            "🕔",
            "🕕",
            "🕖",
            "🕗",
            "🕘",
            "🕙",
            "🕚",
            "🕛",
            "🕜",
            "🕝",
            "🕞",
            "🕟",
            "🕠",
            "🕡",
            "🕢",
            "🕣",
            "🕤",
            "🕥",
            "🕦",
            "🕧",
            "🔥",
            "✨",
            "🌟",
            "💫",
            "⭐",
            "🌈",
            "☀️",
            "🌤️",
            "⛅",
            "🌥️",
            "☁️",
            "🌦️",
            "🌧️",
            "⛈️",
            "🌩️",
            "🌨️",
            "❄️",
            "☃️",
            "⛄",
            "🌬️",
            "💨",
            "💧",
            "💦",
            "☔",
            "☂️",
            "🌊",
            "🌫️",
        ],
    },
];

pub struct EmojiPicker {
    pub container: GtkBox,
    on_emoji_selected: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    flow: FlowBox,
    category_bar: GtkBox,
    title_label: Label,
    active_category: Rc<RefCell<usize>>,
}

impl EmojiPicker {
    pub fn new() -> Self {
        let container = GtkBox::new(Orientation::Vertical, 0);
        container.add_css_class("emoji-picker");
        container.set_size_request(360, 320);

        let title_label = Label::builder()
            .label(CATEGORIES[0].title)
            .xalign(0.0)
            .css_classes(["emoji-picker-title"])
            .margin_start(12)
            .margin_end(12)
            .margin_top(10)
            .margin_bottom(4)
            .build();

        // Category tabs
        let category_bar = GtkBox::new(Orientation::Horizontal, 2);
        category_bar.add_css_class("emoji-category-bar");
        category_bar.set_margin_start(8);
        category_bar.set_margin_end(8);
        category_bar.set_margin_bottom(6);
        category_bar.set_halign(gtk::Align::Fill);
        category_bar.set_homogeneous(true);

        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_height(240)
            .min_content_width(340)
            .hexpand(true)
            .vexpand(true)
            .build();
        scrolled.add_css_class("emoji-picker-scroll");

        let flow = FlowBox::new();
        flow.add_css_class("emoji-flow");
        flow.set_valign(gtk::Align::Start);
        flow.set_max_children_per_line(8);
        flow.set_min_children_per_line(6);
        flow.set_selection_mode(gtk::SelectionMode::None);
        flow.set_homogeneous(true);
        flow.set_column_spacing(2);
        flow.set_row_spacing(2);
        flow.set_margin_start(6);
        flow.set_margin_end(6);
        flow.set_margin_bottom(8);
        scrolled.set_child(Some(&flow));

        container.append(&title_label);
        container.append(&category_bar);
        container.append(&scrolled);

        let on_emoji_selected: Rc<RefCell<Option<Box<dyn Fn(String)>>>> =
            Rc::new(RefCell::new(None));
        let active_category = Rc::new(RefCell::new(0usize));

        let picker = Self {
            container,
            on_emoji_selected,
            flow,
            category_bar,
            title_label,
            active_category,
        };

        picker.build_category_tabs();
        picker.render_category(0);

        picker
    }

    fn build_category_tabs(&self) {
        while let Some(child) = self.category_bar.first_child() {
            self.category_bar.remove(&child);
        }

        for (idx, cat) in CATEGORIES.iter().enumerate() {
            let btn = Button::builder()
                .label(cat.icon)
                .tooltip_text(cat.title)
                .has_frame(false)
                .build();
            btn.add_css_class("emoji-category-btn");
            if idx == 0 {
                btn.add_css_class("emoji-category-btn-active");
            }

            let flow = self.flow.clone();
            let title_label = self.title_label.clone();
            let bar = self.category_bar.clone();
            let active = self.active_category.clone();
            let on_select = self.on_emoji_selected.clone();
            btn.connect_clicked(move |clicked| {
                *active.borrow_mut() = idx;
                // Update tab highlight
                let mut child = bar.first_child();
                let mut i = 0;
                while let Some(w) = child {
                    if let Ok(b) = w.clone().downcast::<Button>() {
                        if i == idx {
                            b.add_css_class("emoji-category-btn-active");
                        } else {
                            b.remove_css_class("emoji-category-btn-active");
                        }
                    }
                    child = w.next_sibling();
                    i += 1;
                }
                let _ = clicked;
                Self::fill_flow(&flow, idx, on_select.clone());
                title_label.set_label(CATEGORIES[idx].title);
            });

            self.category_bar.append(&btn);
        }
    }

    fn render_category(&self, idx: usize) {
        *self.active_category.borrow_mut() = idx;
        self.title_label.set_label(CATEGORIES[idx].title);
        Self::fill_flow(&self.flow, idx, self.on_emoji_selected.clone());
    }

    fn fill_flow(
        flow: &FlowBox,
        idx: usize,
        on_emoji_selected: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    ) {
        while let Some(child) = flow.first_child() {
            flow.remove(&child);
        }

        let cat = &CATEGORIES[idx.min(CATEGORIES.len() - 1)];
        for emoji in cat.emojis {
            let btn = Button::builder().label(*emoji).has_frame(false).build();
            btn.add_css_class("emoji-btn");
            btn.set_size_request(36, 36);

            let emoji_str = emoji.to_string();
            let cb_ref = on_emoji_selected.clone();
            btn.connect_clicked(move |_| {
                if let Some(cb) = cb_ref.borrow().as_ref() {
                    cb(emoji_str.clone());
                }
            });

            let child = FlowBoxChild::new();
            child.set_child(Some(&btn));
            flow.insert(&child, -1);
        }
    }

    pub fn container(&self) -> &Widget {
        self.container.upcast_ref()
    }

    pub fn on_select(&self, callback: impl Fn(String) + 'static) {
        *self.on_emoji_selected.borrow_mut() = Some(Box::new(callback));
    }
}
