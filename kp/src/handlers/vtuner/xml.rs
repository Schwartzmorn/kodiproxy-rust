use serde::{Serialize, Serializer};

use super::helpers::{rewrite_https_to_http, with_vtuner_parameter};

const XML_DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>";

// TODO Review: I'm not very happy with these structs
//  - no typing
//  - inconsistent use of Option
//  - serialize_with used for normalising (maybe ok)
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ListOfItems {
    pub item_count: Option<usize>,
    pub no_data_cache: bool,
    pub items: Vec<Item>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "ItemType", rename_all_fields = "PascalCase")]
pub(super) enum Item {
    #[serde(rename = "Dir")]
    Directory {
        title: String,
        #[serde(serialize_with = "serialize_navigation_url")]
        url_dir: String,
        #[serde(rename = "UrlDirBackUp", serialize_with = "serialize_navigation_url")]
        url_dir_backup: String,
        dir_count: usize,
    },
    Station {
        station_id: String,
        station_name: String,
        #[serde(serialize_with = "serialize_station_url")]
        station_url: String,
        station_desc: String,
        logo: String,
        station_format: String,
        station_location: Option<String>,
        #[serde(rename = "StationBandWidth")]
        station_bandwidth: Option<String>,
        station_mime: Option<String>,
        relia: &'static str,
        bookmark: String,
    },
    Display {
        display: String,
    },
    Search {
        #[serde(rename = "SearchURL", serialize_with = "serialize_navigation_url")]
        search_url: String,
        #[serde(
            rename = "SearchURLBackUp",
            serialize_with = "serialize_navigation_url"
        )]
        search_url_backup: String,
        search_caption: &'static str,
        search_textbox: String,
        search_button_go: &'static str,
        search_button_cancel: &'static str,
    },
    Previous {
        #[serde(serialize_with = "serialize_navigation_url")]
        url_previous: String,
        #[serde(
            rename = "UrlPreviousBackUp",
            serialize_with = "serialize_navigation_url"
        )]
        url_previous_backup: String,
    },
}

impl ListOfItems {
    pub(super) fn to_xml(&self) -> Result<String, quick_xml::SeError> {
        let wire = XmlListOfItems {
            item_count: self
                .item_count
                .map_or_else(|| "-1".to_owned(), |count| count.to_string()),
            no_data_cache: self.no_data_cache.then_some("Yes"),
            items: &self.items,
        };
        let mut body = String::new();
        let mut serializer = quick_xml::se::Serializer::with_root(&mut body, Some("ListOfItems"))?;
        serializer.expand_empty_elements(true);
        wire.serialize(serializer)?;
        Ok(format!("{XML_DECLARATION}{body}"))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct XmlListOfItems<'a> {
    item_count: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    no_data_cache: Option<&'static str>,
    #[serde(rename = "Item")]
    items: &'a [Item],
}
fn serialize_navigation_url<S>(url: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&with_vtuner_parameter(url))
}

fn serialize_station_url<S>(url: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&rewrite_https_to_http(url))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>";

    #[test]
    fn it_renders_the_declaration_count_and_directory_fields_in_order() {
        let response = ListOfItems {
            item_count: Some(1),
            items: vec![Item::Directory {
                title: "R&D <FM>".to_owned(),
                url_dir: "http://radio.example/country/Germany/".to_owned(),
                url_dir_backup: "http://radio.example/country/Germany/".to_owned(),
                dir_count: 12,
            }],
            ..ListOfItems::default()
        };

        assert_eq!(
            format!(
                "{HEADER}<ListOfItems><ItemCount>1</ItemCount><Item><ItemType>Dir</ItemType><Title>R&amp;D &lt;FM&gt;</Title><UrlDir>http://radio.example/country/Germany/?vtuner=true</UrlDir><UrlDirBackUp>http://radio.example/country/Germany/?vtuner=true</UrlDirBackUp><DirCount>12</DirCount></Item></ListOfItems>"
            ),
            response.to_xml().unwrap()
        );
    }

    #[test]
    fn it_renders_station_none_values_as_empty_elements_and_sets_reliability() {
        let response = ListOfItems {
            item_count: Some(1),
            items: vec![Item::Station {
                station_id: "MY_123456789ABC".to_owned(),
                station_name: "Test Radio".to_owned(),
                station_url: "https://radio.example/stream".to_owned(),
                station_desc: "Local".to_owned(),
                logo: "http://radio.example/vtuner/icon?id=MY_123456789ABC".to_owned(),
                station_format: "Local".to_owned(),
                station_location: None,
                station_bandwidth: None,
                station_mime: None,
                relia: "3",
                bookmark: String::new(),
            }],
            ..ListOfItems::default()
        };

        assert_eq!(
            format!(
                "{HEADER}<ListOfItems><ItemCount>1</ItemCount><Item><ItemType>Station</ItemType><StationId>MY_123456789ABC</StationId><StationName>Test Radio</StationName><StationUrl>http://radio.example/stream</StationUrl><StationDesc>Local</StationDesc><Logo>http://radio.example/vtuner/icon?id=MY_123456789ABC</Logo><StationFormat>Local</StationFormat><StationLocation></StationLocation><StationBandWidth></StationBandWidth><StationMime></StationMime><Relia>3</Relia><Bookmark></Bookmark></Item></ListOfItems>"
            ),
            response.to_xml().unwrap()
        );
    }

    #[test]
    fn it_renders_default_count_empty_results_search_and_previous_items() {
        let response = ListOfItems {
            no_data_cache: true,
            items: vec![
                Item::Display {
                    display: "No stations found".to_owned(),
                },
                Item::Search {
                    search_url: "http://radio.example/search/".to_owned(),
                    search_url_backup: "http://radio.example/search/".to_owned(),
                    search_caption: "Search",
                    search_textbox: String::new(),
                    search_button_go: "Search",
                    search_button_cancel: "Cancel",
                },
                Item::Previous {
                    url_previous: "http://radio.example/vtuner/".to_owned(),
                    url_previous_backup: "http://radio.example/vtuner/".to_owned(),
                },
            ],
            ..ListOfItems::default()
        };

        assert_eq!(
            format!(
                "{HEADER}<ListOfItems><ItemCount>-1</ItemCount><NoDataCache>Yes</NoDataCache><Item><ItemType>Display</ItemType><Display>No stations found</Display></Item><Item><ItemType>Search</ItemType><SearchURL>http://radio.example/search/?vtuner=true</SearchURL><SearchURLBackUp>http://radio.example/search/?vtuner=true</SearchURLBackUp><SearchCaption>Search</SearchCaption><SearchTextbox></SearchTextbox><SearchButtonGo>Search</SearchButtonGo><SearchButtonCancel>Cancel</SearchButtonCancel></Item><Item><ItemType>Previous</ItemType><UrlPrevious>http://radio.example/vtuner/?vtuner=true</UrlPrevious><UrlPreviousBackUp>http://radio.example/vtuner/?vtuner=true</UrlPreviousBackUp></Item></ListOfItems>"
            ),
            response.to_xml().unwrap()
        );
    }
}
