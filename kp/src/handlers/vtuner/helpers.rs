use std::{collections::HashMap, ops::Range};

pub(super) fn station_id_prefix(id: &str) -> Option<&str> {
    station_id_parts(id).map(|(prefix, _)| prefix)
}

pub(super) fn station_id_without_prefix(id: &str) -> Option<&str> {
    station_id_parts(id).map(|(_, value)| value)
}

fn station_id_parts(id: &str) -> Option<(&str, &str)> {
    if id.get(2..3)? != "_" {
        return None;
    }

    let prefix = id.get(..2)?;
    let value = id.get(3..)?;
    if value.is_empty() {
        return None;
    }

    Some((prefix, value))
}

pub(super) fn rewrite_https_to_http(url: &str) -> String {
    match url.strip_prefix("https://") {
        Some(rest) => format!("http://{rest}"),
        None => url.to_owned(),
    }
}

pub(super) fn my_station_id(name: &str, url: &str) -> String {
    let digest = md5::compute(format!("{name}{url}").as_bytes());
    let mut folded = [0_u8; 8];
    for (index, value) in folded.iter_mut().enumerate() {
        *value = digest.0[index] ^ digest.0[index + 8];
    }

    let checksum = (0..8)
        .map(|index| format!("{:02x}", folded[index]))
        .collect::<String>();
    format!("MY_{}", checksum[..12].to_uppercase())
}

pub(super) fn absolute_url(scheme: &str, host: &str, path: &str) -> String {
    let path = if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    };
    format!("{}://{host}{path}", scheme.trim_end_matches("://"))
}

pub(super) fn absolute_url_segments(scheme: &str, host: &str, segments: &[&str]) -> String {
    let scheme = scheme.trim_end_matches("://");
    let mut url = url::Url::parse(&format!("{scheme}://{host}/"))
        .expect("request scheme and host form a valid URL");
    let mut path = url
        .path_segments_mut()
        .expect("HTTP URLs support path segments");
    path.clear();
    path.extend(segments.iter().copied());
    path.push("");
    drop(path);
    url.into()
}

pub(super) fn with_vtuner_parameter(url: &str) -> String {
    let (url_without_fragment, fragment) = url
        .split_once('#')
        .map_or((url, ""), |(url, fragment)| (url, fragment));
    let (base, query) = url_without_fragment
        .split_once('?')
        .map_or((url_without_fragment, None), |(base, query)| {
            (base, Some(query))
        });

    if query.is_some_and(|query| {
        form_urlencoded::parse(query.as_bytes())
            .any(|(key, value)| key == "vtuner" && value == "true")
    }) {
        return url.to_owned();
    }

    let query = match query {
        None | Some("") => "vtuner=true".to_owned(),
        Some(query) if query.ends_with('&') => format!("{query}vtuner=true"),
        Some(query) => format!("{query}&vtuner=true"),
    };
    let fragment = if fragment.is_empty() {
        String::new()
    } else {
        format!("#{fragment}")
    };
    format!("{base}?{query}{fragment}")
}

pub(super) fn page_bounds(parameters: &HashMap<String, String>, item_count: usize) -> Range<usize> {
    let offset = first_parameter(parameters, &["startitems", "startItems", "start"])
        .unwrap_or(1)
        .saturating_sub(1);
    let start = offset.min(item_count);

    let end = first_parameter(parameters, &["enditems", "endItems"])
        .or_else(|| {
            first_parameter(parameters, &["howmany"]).map(|count| offset.saturating_add(count))
        })
        .unwrap_or(item_count)
        .min(item_count);

    if end < start {
        start..start
    } else {
        start..end
    }
}

fn first_parameter(parameters: &HashMap<String, String>, names: &[&str]) -> Option<usize> {
    names
        .iter()
        .find_map(|name| parameters.get(*name)?.parse::<usize>().ok())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn it_splits_station_ids_with_a_two_character_prefix() {
        assert_eq!(Some("RB"), station_id_prefix("RB_station-1"));
        assert_eq!(Some("station-1"), station_id_without_prefix("RB_station-1"));
        assert_eq!(None, station_id_prefix("RB_"));
        assert_eq!(None, station_id_without_prefix("R_station-1"));
        assert_eq!(None, station_id_prefix("RB-station-1"));
    }

    #[test]
    fn it_rewrites_only_https_stream_urls() {
        assert_eq!(
            "http://radio.example/stream",
            rewrite_https_to_http("https://radio.example/stream")
        );
        assert_eq!(
            "http://radio.example/stream",
            rewrite_https_to_http("http://radio.example/stream")
        );
    }

    #[test]
    fn it_builds_absolute_urls_and_appends_the_vtuner_parameter() {
        let directory = absolute_url("http", "radio.example:8080", "vtuner/country/Germany/");
        assert_eq!(
            "http://radio.example:8080/vtuner/country/Germany/?vtuner=true",
            with_vtuner_parameter(&directory)
        );
        assert_eq!(
            "https://radio.example/search/?q=rock&vtuner=true",
            with_vtuner_parameter("https://radio.example/search/?q=rock")
        );
        assert_eq!(
            "https://radio.example/search/?vtuner=true#results",
            with_vtuner_parameter("https://radio.example/search/?vtuner=true#results")
        );
        assert_eq!(
            "https://radio.example/search/?vtuner=true#results",
            with_vtuner_parameter("https://radio.example/search/#results")
        );
        assert_eq!(
            "http://radio.example/vtuner/radiobrowser/country/United%20States/",
            absolute_url_segments(
                "http",
                "radio.example",
                &["vtuner", "radiobrowser", "country", "United States"]
            )
        );
    }

    #[test]
    fn it_applies_paging_alias_priority_and_bounds() {
        let parameters = HashMap::from([
            ("startitems".to_owned(), "2".to_owned()),
            ("startItems".to_owned(), "51".to_owned()),
            ("start".to_owned(), "71".to_owned()),
            ("enditems".to_owned(), "4".to_owned()),
            ("endItems".to_owned(), "55".to_owned()),
            ("howmany".to_owned(), "50".to_owned()),
        ]);
        assert_eq!(1..4, page_bounds(&parameters, 100));
    }

    #[test]
    fn it_uses_howmany_as_a_count_and_clamps_to_available_items() {
        let parameters = HashMap::from([
            ("start".to_owned(), "51".to_owned()),
            ("howmany".to_owned(), "50".to_owned()),
        ]);
        assert_eq!(50..80, page_bounds(&parameters, 80));

        let past_end = HashMap::from([("start".to_owned(), "101".to_owned())]);
        assert_eq!(80..80, page_bounds(&past_end, 80));

        let invalid_end = HashMap::from([
            ("start".to_owned(), "4".to_owned()),
            ("enditems".to_owned(), "2".to_owned()),
        ]);
        assert_eq!(3..3, page_bounds(&invalid_end, 80));
    }

    #[test]
    fn it_derives_the_documented_my_station_checksum() {
        assert_eq!(
            "MY_82F6538FA409",
            my_station_id("Deep House Lounge", "http://198.15.94.34:8006")
        );
    }
}
