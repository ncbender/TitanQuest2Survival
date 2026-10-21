//! TQ2's masteries as exported by `Tq2Extract masteries` (cache/export/masteries/masteries.json):
//! per mastery five tiers of active skills, passives (with feats) and modifiers, with their
//! original English names, descriptions and icons. Also turns TQ2's text markup into coloured
//! spans and fills its `{placeholders}` with the game's own values.
use bevy::prelude::*;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct MasteryInfo {
    pub id: String,
    pub name: String,
    pub slogan: String,
    pub description: String,
    /// Hex colour, e.g. "4A6379".
    pub color: String,
    /// Artwork (e.g. the storm mage) and background texture names.
    pub icon: Option<String>,
    pub background: Option<String>,
    pub tiers: Vec<TierInfo>,
}

#[derive(Deserialize, Clone)]
pub struct TierInfo {
    pub actives: Vec<SkillInfo>,
    pub passives: Vec<PassiveInfo>,
    pub modifiers: Vec<SkillInfo>,
}

#[derive(Deserialize, Clone)]
pub struct SkillInfo {
    pub id: String,
    /// The skill folder in TQ2's data (modifiers share it with their skill).
    pub group: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    /// TQ2 hides these behind a work-in-progress entitlement.
    pub wip: bool,
}

#[derive(Deserialize, Clone)]
pub struct PassiveInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub max_level: u32,
    pub feats: Vec<FeatInfo>,
}

#[derive(Deserialize, Clone)]
pub struct FeatInfo {
    pub name: String,
    pub description: String,
}

#[derive(Resource, Default)]
pub struct MasteryBook(pub Vec<MasteryInfo>);

/// Where the mastery textures (icons, artwork) live below the asset root.
pub fn texture(name: &str) -> String {
    format!("masteries/textures/{name}.png")
}

/// TQ2 UI textures (frames, backgrounds) below the asset root.
pub fn ui_texture(name: &str) -> String {
    format!("ui/textures/{name}.png")
}

impl MasteryBook {
    pub fn load(root: &str) -> Self {
        let path = format!("{root}/masteries/masteries.json");
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string())) {
            Ok(list) => MasteryBook(list),
            Err(e) => {
                warn!("no mastery data at {path} ({e}); run `Tq2Extract assets masteries`");
                MasteryBook::default()
            }
        }
    }

    pub fn mastery(&self, id: &str) -> Option<&MasteryInfo> {
        self.0.iter().find(|m| m.id == id)
    }

    /// The mastery and tier an active skill belongs to.
    pub fn active(&self, id: &str) -> Option<(&MasteryInfo, usize, &SkillInfo)> {
        self.0.iter().find_map(|m| {
            m.tiers.iter().enumerate().find_map(|(t, tier)| tier.actives.iter().find(|a| a.id == id).map(|a| (m, t, a)))
        })
    }

    pub fn passive(&self, id: &str) -> Option<(&MasteryInfo, usize, &PassiveInfo)> {
        self.0.iter().find_map(|m| {
            m.tiers.iter().enumerate().find_map(|(t, tier)| tier.passives.iter().find(|p| p.id == id).map(|p| (m, t, p)))
        })
    }

    /// TQ2's modifiers for a skill (same data folder), from every tier of its mastery.
    pub fn modifiers<'a>(&'a self, skill: &'a SkillInfo) -> impl Iterator<Item = &'a SkillInfo> + 'a {
        self.0.iter().flat_map(|m| m.tiers.iter()).flat_map(|t| t.modifiers.iter()).filter(move |m| m.group == skill.group)
    }
}

impl MasteryInfo {
    pub fn color(&self) -> Color {
        Srgba::hex(&self.color).map(Color::from).unwrap_or(Color::srgb(0.5, 0.5, 0.5))
    }
}

pub const GOLD: Color = Color::srgb(0.93, 0.78, 0.42);
pub const TEXT: Color = Color::srgb(0.88, 0.85, 0.78);
pub const HIGHLIGHT: Color = Color::WHITE;

/// Colour of a `<me_…>` term (damage types, ailments).
fn term_color(tag: &str) -> Color {
    let t = tag.to_ascii_lowercase();
    if t.contains("fire") || t.contains("burn") {
        Color::srgb(1.0, 0.55, 0.25)
    } else if t.contains("cold") || t.contains("chill") || t.contains("freez") || t.contains("frost") {
        Color::srgb(0.55, 0.85, 1.0)
    } else if t.contains("lightning") || t.contains("shock") {
        Color::srgb(0.75, 0.8, 1.0)
    } else if t.contains("poison") || t.contains("toxic") {
        Color::srgb(0.55, 0.9, 0.4)
    } else if t.contains("vitality") || t.contains("spirit") || t.contains("soul") {
        Color::srgb(0.5, 0.95, 0.85)
    } else if t.contains("pierce") || t.contains("strike") || t.contains("physical") || t.contains("bleed") {
        Color::srgb(0.9, 0.82, 0.7)
    } else {
        GOLD
    }
}

/// Splits TQ2 markup into coloured spans. `<b>` = white, `<b_gold>` = gold, `<me_X>` = the
/// term's colour, `</>` closes. Placeholders `{Name}|fmt(…)` are replaced by `value(Name)`
/// (with "data." prefixes and "_UI" suffixes stripped), or "–" when the game has no value.
pub fn render(text: &str, value: &dyn Fn(&str) -> Option<f32>) -> Vec<(String, Color)> {
    let text = text.replace("\\r\\n", "\n").replace("\\n", "\n").replace("\r\n", "\n");
    let mut spans: Vec<(String, Color)> = vec![];
    let mut stack = vec![TEXT];
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let push = |spans: &mut Vec<(String, Color)>, s: &mut String, c: Color| {
        if !s.is_empty() {
            spans.push((std::mem::take(s), c));
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '<' {
            if let Some(end) = chars[i..].iter().position(|&c| c == '>') {
                let tag: String = chars[i + 1..i + end].iter().collect();
                push(&mut spans, &mut current, *stack.last().unwrap());
                if tag == "/" {
                    if stack.len() > 1 {
                        stack.pop();
                    }
                } else if tag == "b" {
                    stack.push(HIGHLIGHT);
                } else if tag.starts_with("b_gold") {
                    stack.push(GOLD);
                } else if let Some(term) = tag.strip_prefix("me_") {
                    stack.push(term_color(term));
                } else {
                    stack.push(*stack.last().unwrap());
                }
                i += end + 1;
                continue;
            }
        }
        if c == '{' {
            if let Some(end) = chars[i..].iter().position(|&c| c == '}') {
                let name: String = chars[i + 1..i + end].iter().collect();
                let mut next = i + end + 1;
                // Skip a formatter like |round() or |fmt(min_frac=1,max_frac=1).
                let (mut integer, mut percent, mut sign) = (false, false, false);
                if chars.get(next) == Some(&'|') {
                    if let Some(close) = chars[next..].iter().position(|&c| c == ')') {
                        let f: String = chars[next..next + close].iter().collect();
                        integer = f.contains("round") || f.contains("floor") || f.contains("max_frac=0");
                        // "perc" shows a value as percent (the game's values already are percent).
                        percent = f.contains("perc");
                        sign = f.contains("sign=true");
                        next += close + 1;
                    }
                }
                let v = value(&key(&name));
                if sign && v.is_some_and(|v| v > 0.0) {
                    current.push('+');
                }
                current.push_str(&format_value(v, integer));
                if percent {
                    current.push('%');
                }
                i = next;
                continue;
            }
        }
        current.push(c);
        i += 1;
    }
    push(&mut spans, &mut current, *stack.last().unwrap());
    spans
}

/// "data.Radius_UI" -> "Radius", "dataBase.MinDmg" -> "MinDmg".
pub fn key(name: &str) -> String {
    let name = name.rsplit('.').next().unwrap_or(name);
    name.trim_end_matches("_UI").to_string()
}

fn format_value(v: Option<f32>, integer: bool) -> String {
    match v {
        None => "–".into(),
        Some(v) if integer || (v - v.round()).abs() < 0.05 => format!("{}", v.round() as i64),
        Some(v) => format!("{v:.1}"),
    }
}

/// The text without markup.
#[cfg(test)]
pub fn plain(text: &str, value: &dyn Fn(&str) -> Option<f32>) -> String {
    render(text, value).into_iter().map(|(s, _)| s).collect()
}

pub fn plugin(app: &mut App) {
    app.insert_resource(MasteryBook::load(&crate::export_root()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_markup_and_values() {
        let text = "Deals <b>{data.Damage}|round()</> <me_FireDam>Fire Damage</> in a <b>{data.Radius_UI}|fmt(min_frac=1,max_frac=1)m</> radius.\\nNext";
        let v = |k: &str| match k {
            "Damage" => Some(34.4),
            "Radius" => Some(3.5),
            _ => None,
        };
        assert_eq!(plain(text, &v), "Deals 34 Fire Damage in a 3.5m radius.\nNext");
        let spans = render(text, &v);
        assert!(spans.iter().any(|(s, c)| s == "Fire Damage" && *c != TEXT));
        assert_eq!(plain("{Unknown}|floor()%", &|_| None), "–%");
    }
}
