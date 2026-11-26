//! RSS and Atom feeds to links. Feeds in the wild are often sloppy, so parsing is tolerant: a
//! feed that breaks half way still yields the entries read so far.

use super::clean::{clean_title, resolve_link};
use crate::state::{LiveItem, LIVE_ITEMS_MAX};
use anyhow::{bail, Result};
use quick_xml::{
    events::{BytesStart, Event},
    Reader,
};

/// The text field of an entry being read.
#[derive(Clone, Copy, PartialEq)]
enum Field {
    Title,
    Link,
}

/// One `<item>` (RSS) or `<entry>` (Atom) while it is being read.
#[derive(Default)]
struct Entry {
    title: String,
    /// Atom keeps the address in an `href` attribute.
    href: String,
    /// RSS keeps it as the element's text.
    link_text: String,
    /// Elements open inside the entry; fields are only read at depth 1, so a nested
    /// `<source><title>` is not mistaken for the entry's title.
    depth: usize,
    field: Option<Field>,
}

impl Entry {
    fn start_child(&mut self, tag: &BytesStart) {
        self.depth += 1;
        if self.depth != 1 {
            return;
        }
        self.field = match tag.local_name().as_ref() {
            b"title" => Some(Field::Title),
            b"link" => {
                self.take_href(tag);
                Some(Field::Link)
            }
            _ => None,
        };
    }

    /// Reads `href` from an Atom `<link>`, unless it points at something other than the page.
    fn take_href(&mut self, tag: &BytesStart) {
        let mut href = None;
        let mut alternate = true;
        for attribute in tag.attributes().flatten() {
            let value = String::from_utf8_lossy(&attribute.value).into_owned();
            match attribute.key.as_ref() {
                b"href" => href = Some(value),
                b"rel" => alternate = value == "alternate",
                _ => {}
            }
        }
        if let (true, true, Some(href)) = (alternate, self.href.is_empty(), href) {
            self.href = href;
        }
    }

    fn push_text(&mut self, text: &str) {
        match self.field {
            Some(Field::Title) => self.title.push_str(text),
            Some(Field::Link) => self.link_text.push_str(text),
            None => {}
        }
    }

    fn finish(self, base: &str) -> Option<LiveItem> {
        let link = if self.href.is_empty() {
            self.link_text
        } else {
            self.href
        };
        let url = resolve_link(base, &link)?;
        Some(LiveItem {
            title: clean_title(&self.title, &url),
            url,
        })
    }
}

/// The entries of the feed in `xml`; `base` (the feed's address) resolves relative links.
pub fn parse(xml: &str, base: &str) -> Result<Vec<LiveItem>> {
    let mut reader = Reader::from_str(xml);
    let config = reader.config_mut();
    config.check_end_names = false;
    config.trim_text(true);

    let mut items: Vec<LiveItem> = Vec::new();
    let mut entry: Option<Entry> = None;
    while items.len() < LIVE_ITEMS_MAX {
        match reader.read_event() {
            Ok(Event::Start(tag)) => match entry.as_mut() {
                Some(open) => open.start_child(&tag),
                None if matches!(tag.local_name().as_ref(), b"item" | b"entry") => {
                    entry = Some(Entry::default());
                }
                None => {}
            },
            Ok(Event::Empty(tag)) => {
                if let Some(open) = entry.as_mut().filter(|e| e.depth == 0) {
                    if tag.local_name().as_ref() == b"link" {
                        open.take_href(&tag);
                    }
                }
            }
            Ok(Event::Text(text)) => {
                if let Some(open) = entry.as_mut().filter(|e| e.depth == 1) {
                    match text.unescape() {
                        Ok(text) => open.push_text(&text),
                        Err(_) => open.push_text(&String::from_utf8_lossy(&text)),
                    }
                }
            }
            Ok(Event::CData(data)) => {
                if let Some(open) = entry.as_mut().filter(|e| e.depth == 1) {
                    open.push_text(&String::from_utf8_lossy(&data));
                }
            }
            Ok(Event::End(_)) => {
                if let Some(open) = entry.as_mut() {
                    if open.depth > 0 {
                        if open.depth == 1 {
                            open.field = None;
                        }
                        open.depth -= 1;
                    } else if let Some(item) = entry.take().and_then(|e| e.finish(base)) {
                        if !items.iter().any(|known| known.url == item.url) {
                            items.push(item);
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    if items.is_empty() {
        bail!("This address does not look like an RSS or Atom feed.");
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://example.com/blog/feed.xml";

    fn pairs(items: &[LiveItem]) -> Vec<(&str, &str)> {
        items
            .iter()
            .map(|i| (i.title.as_str(), i.url.as_str()))
            .collect()
    }

    #[test]
    fn reads_rss_with_cdata_entities_and_relative_links() {
        let xml = r#"<?xml version="1.0"?>
        <rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom"><channel>
          <title>The blog</title><link>https://example.com/</link>
          <atom:link href="https://example.com/feed.xml" rel="self"/>
          <item><title><![CDATA[Fish & <b>chips</b>]]></title><link>https://example.com/a</link></item>
          <item><title>Tom &amp; Jerry</title><link>/blog/b</link></item>
        </channel></rss>"#;
        let items = parse(xml, BASE).unwrap();
        assert_eq!(
            pairs(&items),
            vec![
                ("Fish & chips", "https://example.com/a"),
                ("Tom & Jerry", "https://example.com/blog/b"),
            ]
        );
    }

    #[test]
    fn reads_atom_and_ignores_non_page_links_and_nested_titles() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>Feed</title>
          <entry>
            <title>First</title>
            <link rel="replies" href="https://example.com/replies"/>
            <link rel="alternate" href="https://example.com/first"/>
            <source><title>Elsewhere</title></source>
          </entry>
          <entry><title type="html">Second</title><link href="second"/></entry>
        </feed>"#;
        let items = parse(xml, BASE).unwrap();
        assert_eq!(
            pairs(&items),
            vec![
                ("First", "https://example.com/first"),
                ("Second", "https://example.com/blog/second"),
            ]
        );
    }

    #[test]
    fn drops_non_web_links_duplicates_and_untitled_entries_get_their_address() {
        let xml = r#"<rss><channel>
          <item><title>Bad</title><link>javascript:alert(1)</link></item>
          <item><title>Mail</title><link>mailto:a@b.c</link></item>
          <item><title>Good</title><link>https://example.com/x</link></item>
          <item><title>Again</title><link>https://example.com/x</link></item>
          <item><link>https://example.com/untitled</link></item>
        </channel></rss>"#;
        let items = parse(xml, BASE).unwrap();
        assert_eq!(
            pairs(&items),
            vec![
                ("Good", "https://example.com/x"),
                ("example.com/untitled", "https://example.com/untitled"),
            ]
        );
    }

    #[test]
    fn a_feed_that_breaks_half_way_keeps_what_it_had() {
        let xml = "<rss><channel><item><title>One</title><link>https://example.com/1</link></item><item><title>Two</ti";
        assert_eq!(parse(xml, BASE).unwrap().len(), 1);
    }

    #[test]
    fn pages_that_are_not_feeds_are_refused() {
        assert!(parse("<html><body><p>hello</p></body></html>", BASE).is_err());
        assert!(parse("", BASE).is_err());
    }

    #[test]
    fn stops_at_the_item_limit() {
        let many: String = (0..100)
            .map(|n| {
                format!("<item><title>T{n}</title><link>https://example.com/{n}</link></item>")
            })
            .collect();
        let items = parse(&format!("<rss><channel>{many}</channel></rss>"), BASE).unwrap();
        assert_eq!(items.len(), LIVE_ITEMS_MAX);
    }
}
