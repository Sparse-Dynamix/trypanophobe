use regex::Regex;
use std::sync::LazyLock;

static RE_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://\S+|www\.\S+").expect("url regex"));
static RE_HTML: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").expect("html regex"));
static RE_MENTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"@\w+").expect("mention regex"));
static RE_DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").expect("digit regex"));
static RE_PUNCT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\w\s]").expect("punct regex"));
static RE_BRACKETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\[\](){}<>]").expect("bracket regex"));

pub fn preprocess_for_nsfw_text(input: &str) -> String {
    let mut s = input.to_ascii_lowercase();
    s = RE_URL.replace_all(&s, " ").into_owned();
    s = RE_HTML.replace_all(&s, " ").into_owned();
    s = RE_MENTION.replace_all(&s, " ").into_owned();
    s = RE_DIGITS.replace_all(&s, " ").into_owned();
    s = RE_BRACKETS.replace_all(&s, " ").into_owned();
    s = RE_PUNCT.replace_all(&s, " ").into_owned();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
