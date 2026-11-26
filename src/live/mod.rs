//! Fetching the links of a live folder from its source. Blocking: call from a worker thread,
//! never from the event loop. Everything fetched is untrusted and cleaned before it is returned.

mod clean;
mod feed;
mod github;

use crate::state::{LiveItem, LiveSource};
use anyhow::{anyhow, bail, Result};
use std::{io::Read, time::Duration};

/// A feed or search answer larger than this is not what it claims to be.
const MAX_BODY_BYTES: u64 = 2 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_REDIRECTS: u32 = 4;

pub fn fetch(source: &LiveSource) -> Result<Vec<LiveItem>> {
    match source {
        LiveSource::Feed { url } => {
            let body = download(
                url,
                "application/rss+xml, application/atom+xml, text/xml, */*",
            )?;
            feed::parse(&body, url)
        }
        LiveSource::Github { query } => {
            let body = download(&github::search_url(query)?, "application/vnd.github+json")?;
            github::parse(&body)
        }
    }
}

fn download(url: &str, accept: &str) -> Result<String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout_read(READ_TIMEOUT)
        .redirects(MAX_REDIRECTS)
        .user_agent(concat!("poof/", env!("CARGO_PKG_VERSION")))
        .build();
    let response = match agent.get(url).set("Accept", accept).call() {
        Ok(response) => response,
        Err(ureq::Error::Status(code, _)) => bail!(describe_status(code)),
        Err(ureq::Error::Transport(_)) => bail!("The server could not be reached."),
    };
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_BODY_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow!("The download was interrupted."))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// What an HTTP error means for the user.
fn describe_status(code: u16) -> String {
    match code {
        404 | 410 => "Nothing was found at this address.".into(),
        401 | 403 | 429 => "The server refused the request or its rate limit was reached; poof will try again later.".into(),
        422 => "The server could not run this search. Check the query.".into(),
        500..=599 => "The server had a problem; poof will try again later.".into(),
        other => format!("The server answered with error {other}."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_errors_are_explained_in_plain_words() {
        assert!(describe_status(404).contains("Nothing was found"));
        assert!(describe_status(403).contains("rate limit"));
        assert!(describe_status(503).contains("try again"));
        assert!(describe_status(418).contains("418"));
    }
}
