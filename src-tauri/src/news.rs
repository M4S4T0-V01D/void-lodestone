//! Game news from Jagex's RSS feeds and whether the game servers are up. Only fetched while the News tab is open.

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;
use std::time::Duration;

const OSRS_FEED: &str = "https://secure.runescape.com/m=news/latest_news.rss?oldschool=true";
const RS3_FEED: &str = "https://secure.runescape.com/m=news/latest_news.rss";
const OSRS_WORLDS: &str = "https://oldschool.runescape.com/slr.ws?order=LPWM";
const PLAYER_COUNT: &str = "https://www.runescape.com/player_count.js?varname=iPlayerCount&callback=jQuery000000000000000_0000000000000";
const SIGN_IN: &str = "https://account.jagex.com/";

/// Links the window may open in the browser.
const NEWS_HOSTS: [&str; 5] =
    ["secure.runescape.com", "oldschool.runescape.com", "www.runescape.com", "runescape.com", "support.runescape.com"];

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub title: String,
    pub link: String,
    pub summary: String,
    /// As the feed gives it (RFC 822).
    pub date: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Old School worlds listed and how many of them are up; None if the list couldn't be fetched.
    pub osrs_worlds: Option<u32>,
    pub osrs_online: Option<u32>,
    pub osrs_players: Option<u64>,
    /// Everyone playing either game.
    pub all_players: Option<u64>,
    /// Jagex's sign-in site answered.
    pub sign_in: bool,
    pub checked_at: u64,
}

pub fn feed_url(game: &str) -> &'static str {
    if game == "rs3" { RS3_FEED } else { OSRS_FEED }
}

pub async fn feed(http: &reqwest::Client, game: &str) -> Result<Vec<Item>, String> {
    let body = get(http, feed_url(game)).await?.text().await.map_err(|e| format!("couldn't read the news: {e}"))?;
    parse_feed(&body)
}

async fn get(http: &reqwest::Client, url: &str) -> Result<reqwest::Response, String> {
    let res = http.get(url).timeout(Duration::from_secs(15)).send().await.map_err(|e| format!("couldn't reach Jagex: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("Jagex answered {}", res.status()));
    }
    Ok(res)
}

pub fn parse_feed(xml: &str) -> Result<Vec<Item>, String> {
    let mut r = Reader::from_str(xml);
    let mut items = vec![];
    let mut cur: Option<Item> = None;
    let mut text = String::new();
    loop {
        match r.read_event().map_err(|e| format!("the news feed couldn't be read: {e}"))? {
            Event::Start(e) => {
                if e.name().as_ref() == "item" {
                    cur = Some(Item::default());
                }
                text.clear();
            }
            Event::Text(t) => text.push_str(&t.into_inner()),
            Event::CData(t) => text.push_str(&t.into_inner()),
            Event::GeneralRef(g) => {
                if let Ok(Some(c)) = g.resolve_char_ref() {
                    text.push(c);
                } else {
                    let name = g.into_inner();
                    text.push_str(match name.as_ref() {
                        "amp" => "&",
                        "lt" => "<",
                        "gt" => ">",
                        "quot" => "\"",
                        "apos" => "'",
                        "nbsp" => " ",
                        _ => "",
                    });
                }
            }
            Event::End(e) => {
                let name = e.name();
                let name = name.as_ref();
                if let Some(it) = cur.as_mut() {
                    let v = text.trim().to_string();
                    match name {
                        "title" => it.title = v,
                        "link" => it.link = v,
                        "description" => it.summary = strip_html(&v),
                        "pubDate" => it.date = v,
                        "category" => it.category = v,
                        "item" => items.extend(cur.take()),
                        _ => {}
                    }
                }
                text.clear();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    items.retain(|i| !i.title.is_empty());
    items.truncate(30);
    Ok(items)
}

/// Descriptions may carry markup; the window shows them as plain text.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' if tag => {
                tag = false;
                out.push(' ');
            }
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    let out = out.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'");
    out.replace("&amp;", "&").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The Old School world list: how many worlds, how many are up, and the players on them.
pub fn parse_worlds(b: &[u8]) -> Option<(u32, u32, u64)> {
    let mut p = 4; // the body's size
    let take = |p: &mut usize, n: usize| -> Option<&[u8]> {
        let s = b.get(*p..*p + n)?;
        *p += n;
        Some(s)
    };
    let cstr = |p: &mut usize| -> Option<()> {
        let end = b.get(*p..)?.iter().position(|c| *c == 0)?;
        *p += end + 1;
        Some(())
    };
    let count = u16::from_be_bytes(take(&mut p, 2)?.try_into().ok()?);
    let (mut online, mut players) = (0, 0u64);
    for _ in 0..count {
        take(&mut p, 2 + 4)?; // id, flags
        cstr(&mut p)?; // host
        cstr(&mut p)?; // activity
        take(&mut p, 1)?; // location
        let n = i16::from_be_bytes(take(&mut p, 2)?.try_into().ok()?);
        if n >= 0 {
            online += 1;
            players += n as u64;
        }
    }
    Some((count as u32, online, players))
}

/// `jQuery(12345);` → 12345.
fn parse_count(s: &str) -> Option<u64> {
    let inner = &s[s.find('(')? + 1..s.rfind(')')?];
    inner.trim().parse().ok()
}

pub async fn status(http: &reqwest::Client) -> Status {
    let worlds = async {
        let b = get(http, OSRS_WORLDS).await.ok()?.bytes().await.ok()?;
        parse_worlds(&b)
    };
    let count = async { parse_count(&get(http, PLAYER_COUNT).await.ok()?.text().await.ok()?) };
    let sign_in = async {
        http.get(SIGN_IN).timeout(Duration::from_secs(10)).send().await.is_ok_and(|r| !r.status().is_server_error())
    };
    let (w, all_players, sign_in) = tokio::join!(worlds, count, sign_in);
    Status {
        osrs_worlds: w.map(|w| w.0),
        osrs_online: w.map(|w| w.1),
        osrs_players: w.map(|w| w.2),
        all_players,
        sign_in,
        checked_at: crate::config::now(),
    }
}

/// Only Jagex's news pages, over https.
pub fn may_open(link: &str) -> bool {
    url::Url::parse(link).is_ok_and(|u| u.scheme() == "https" && u.host_str().is_some_and(|h| NEWS_HOSTS.contains(&h)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_feed() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><rss version="2.0"><channel><title>News</title>
            <item><title>Varlamore &amp; More</title><link>https://secure.runescape.com/m=news/varlamore?oldschool=1</link>
            <description><![CDATA[<p>New <b>area</b> out&nbsp;now.</p>]]></description><category>Game Updates</category>
            <pubDate>Wed, 11 Nov 2026 11:00:00 GMT</pubDate><enclosure url="https://cdn.example/x.png" type="image/png"/></item>
            <item><title>Second</title><description>Plain &#8217;text&#8217;</description></item>
            </channel></rss>"#;
        let items = parse_feed(xml).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Varlamore & More");
        assert_eq!(items[0].summary, "New area out now.");
        assert_eq!(items[0].category, "Game Updates");
        assert!(items[0].date.starts_with("Wed"));
        assert_eq!(items[1].summary, "Plain \u{2019}text\u{2019}");
        assert!(parse_feed("<rss><item>").unwrap().is_empty());
    }

    #[test]
    fn reads_the_world_list() {
        let mut b = vec![0, 0, 0, 0, 0, 2];
        for (id, players) in [(301u16, 812i16), (302, -1)] {
            b.extend(id.to_be_bytes());
            b.extend(1u32.to_be_bytes());
            b.extend(b"oldschool1.runescape.com\0Trade\0");
            b.push(0);
            b.extend(players.to_be_bytes());
        }
        assert_eq!(parse_worlds(&b), Some((2, 1, 812)));
        assert_eq!(parse_worlds(&b[..b.len() - 1]), None);
        assert_eq!(parse_count("jQuery(123456);"), Some(123456));
        assert_eq!(parse_count("nope"), None);
    }

    /// Needs the network: `cargo test -- --ignored live_news --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn live_news() {
        let http = reqwest::Client::builder().user_agent("VOID Lodestone test").build().unwrap();
        for g in ["osrs", "rs3"] {
            let items = feed(&http, g).await.unwrap();
            println!("{g}: {} items, first {:?}", items.len(), items.first());
            assert!(!items.is_empty());
        }
        println!("{:?}", status(&http).await);
    }

    #[test]
    fn opens_only_jagex_news() {
        assert!(may_open("https://secure.runescape.com/m=news/a?oldschool=1"));
        assert!(!may_open("http://secure.runescape.com/m=news/a"));
        assert!(!may_open("https://secure.runescape.com.evil.example/"));
        assert!(!may_open("file:///etc/passwd"));
    }
}
