//! `GET /v1/vms`: the options of `vm list` that the Host CLI parsed.
//!
//! They travel in the query: `limit=N`, `all=true` and one `filter=field%3Dvalue` for each
//! pair of `--filter`. The parser and the builder live together, so a test can prove that they
//! agree. The response is the view that `vm list` prints, as JSON (spec: "The protocol").

use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};

/// The path of the request.
pub const LIST_PATH: &str = "/v1/vms";

/// What `vm list` asks for: the limit, and the pairs of `--filter`, already parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRequest {
    pub limit: usize,
    pub all: bool,
    pub filters: Vec<(String, String)>,
}

impl ListRequest {
    /// The path and query of the request.
    pub fn path_and_query(&self) -> String {
        let mut query = vec![format!("limit={}", self.limit)];
        if self.all {
            query.push("all=true".to_string());
        }
        for (field, value) in &self.filters {
            let pair = format!("{field}={value}");
            query.push(format!(
                "filter={}",
                utf8_percent_encode(&pair, NON_ALPHANUMERIC)
            ));
        }
        format!("{LIST_PATH}?{}", query.join("&"))
    }

    /// Read the query of a request. An error names the part that is wrong.
    pub fn parse(query: Option<&str>) -> Result<Self, String> {
        let mut limit = None;
        let mut all = false;
        let mut filters = Vec::new();
        for part in query.unwrap_or("").split('&').filter(|p| !p.is_empty()) {
            let (name, value) = part.split_once('=').unwrap_or((part, ""));
            match name {
                "limit" => {
                    limit = Some(
                        value
                            .parse::<usize>()
                            .map_err(|_| format!("limit '{value}' is not a number"))?,
                    );
                }
                "all" => {
                    all = match value {
                        "true" => true,
                        "false" => false,
                        _ => return Err(format!("all '{value}' is not true or false")),
                    };
                }
                "filter" => {
                    let pair = percent_decode_str(value).decode_utf8_lossy().into_owned();
                    let (field, value) = pair
                        .split_once('=')
                        .ok_or_else(|| format!("filter '{pair}' is not a field=value pair"))?;
                    filters.push((field.to_string(), value.to_string()));
                }
                other => return Err(format!("'{other}' is no option of the listing")),
            }
        }
        let limit = limit.ok_or_else(|| "the request states no limit".to_string())?;
        Ok(Self {
            limit,
            all,
            filters,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(request: &ListRequest) -> ListRequest {
        let target = request.path_and_query();
        let (path, query) = target.split_once('?').expect("a query");
        assert_eq!(path, LIST_PATH);
        ListRequest::parse(Some(query)).expect("the query reads")
    }

    #[test]
    fn a_request_round_trips_through_its_query() {
        for request in [
            ListRequest {
                limit: 100,
                all: false,
                filters: vec![],
            },
            ListRequest {
                limit: 0,
                all: true,
                filters: vec![("platform".into(), "windows".into())],
            },
            ListRequest {
                limit: 5,
                all: false,
                // A value may hold '=', '&', '%' and a space.
                filters: vec![
                    ("name".into(), "a=b&c%20d e".into()),
                    ("kind".into(), "running".into()),
                ],
            },
        ] {
            assert_eq!(round_trip(&request), request);
        }
    }

    #[test]
    fn a_filter_pair_is_one_encoded_parameter() {
        let request = ListRequest {
            limit: 100,
            all: false,
            filters: vec![("platform".into(), "windows".into())],
        };
        assert_eq!(
            request.path_and_query(),
            "/v1/vms?limit=100&filter=platform%3Dwindows"
        );
    }

    #[test]
    fn a_query_without_a_limit_is_refused() {
        assert!(ListRequest::parse(None).unwrap_err().contains("no limit"));
        assert!(ListRequest::parse(Some("all=true"))
            .unwrap_err()
            .contains("no limit"));
    }

    #[test]
    fn a_query_with_a_wrong_part_names_it() {
        for (query, wrong) in [
            ("limit=ten", "limit 'ten'"),
            ("limit=1&all=yes", "all 'yes'"),
            ("limit=1&filter=platform", "filter 'platform'"),
            ("limit=1&colour=blue", "'colour'"),
        ] {
            let error = ListRequest::parse(Some(query)).unwrap_err();
            assert!(error.contains(wrong), "{query}: {error}");
        }
    }
}
