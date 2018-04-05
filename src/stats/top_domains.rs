/* Pi-hole: A black hole for Internet advertisements
*  (c) 2018 Pi-hole, LLC (https://pi-hole.net)
*  Network-wide ad blocking via your own hardware.
*
*  API
*  Top Domains/Blocked Endpoints
*
*  This file is copyright under the latest version of the EUPL.
*  Please see LICENSE file for your rights under this license. */

use ftl::FtlConnectionType;
use rmp::decode::DecodeStringError;
use rmp::Marker;
use rocket::State;
use std::collections::HashMap;
use util;

/// Return the top domains with default parameters
#[get("/stats/top_domains")]
pub fn top_domains(ftl: State<FtlConnectionType>) -> util::Reply {
    get_top_domains(&ftl, false, TopParams::default())
}

/// Return the top domains with specified parameters
#[get("/stats/top_domains?<params>")]
pub fn top_domains_params(ftl: State<FtlConnectionType>, params: TopParams) -> util::Reply {
    get_top_domains(&ftl, false, params)
}

/// Return the top blocked domains with default parameters
#[get("/stats/top_blocked")]
pub fn top_blocked(ftl: State<FtlConnectionType>) -> util::Reply {
    get_top_domains(&ftl, true, TopParams::default())
}

/// Return the top blocked domains with specified parameters
#[get("/stats/top_blocked?<params>")]
pub fn top_blocked_params(ftl: State<FtlConnectionType>, params: TopParams) -> util::Reply {
    get_top_domains(&ftl, true, params)
}

/// Represents the possible GET parameters on `/stats/top_domains` and `/stats/top_blocked`
#[derive(FromForm)]
pub struct TopParams {
    limit: Option<usize>,
    audit: Option<bool>,
    ascending: Option<bool>,
}

impl Default for TopParams {
    /// The default parameters of top_domains and top_blocked requests
    fn default() -> Self {
        TopParams {
            limit: Some(10),
            audit: Some(false),
            ascending: Some(false),
        }
    }
}

/// Get the top domains (blocked or not)
fn get_top_domains(ftl: &FtlConnectionType, blocked: bool, params: TopParams) -> util::Reply {
    let default_limit: usize = TopParams::default().limit.unwrap_or(10);

    // Create the command to send to FTL
    let command = format!(
        "{} ({}){}{}",
        if blocked { "top-ads" } else { "top-domains" },
        params.limit.unwrap_or(default_limit),
        if params.audit.unwrap_or(false) {
            " for audit"
        } else {
            ""
        },
        if params.ascending.unwrap_or(false) {
            " asc"
        } else {
            ""
        }
    );

    // Connect to FTL
    let mut con = ftl.connect(&command)?;

    // Read the number of queries (blocked or total)
    let queries = con.read_i32()?;

    // Create a 4KiB string buffer
    let mut str_buffer = [0u8; 4096];

    // Store the domain -> number data here
    let mut top: HashMap<String, i32> = HashMap::new();

    // Read in the data
    loop {
        // Get the domain, unless we are at the end of the list
        let domain = match con.read_str(&mut str_buffer) {
            Ok(domain) => domain,
            Err(e) => {
                // Check if we received the EOM
                if let DecodeStringError::TypeMismatch(marker) = e {
                    if marker == Marker::Reserved {
                        // Received EOM
                        break;
                    }
                }

                // Unknown read error
                return util::reply_error(util::Error::Unknown);
            }
        };

        let count = con.read_i32()?;

        top.insert(domain.to_string(), count);
    }

    // Get the keys to send the data under
    let (top_type, queries_type) = if blocked {
        ("top_blocked", "blocked_queries")
    } else {
        ("top_domains", "total_queries")
    };

    util::reply_data(json!({
        top_type: top,
        queries_type: queries
    }))
}

#[cfg(test)]
mod test {
    use rmp::encode;
    use testing::{TestBuilder, write_eom};

    #[test]
    fn test_top_domains() {
        let mut data = Vec::new();
        encode::write_i32(&mut data, 10).unwrap();
        encode::write_str(&mut data, "example.com").unwrap();
        encode::write_i32(&mut data, 7).unwrap();
        encode::write_str(&mut data, "example.net").unwrap();
        encode::write_i32(&mut data, 3).unwrap();
        write_eom(&mut data);

        TestBuilder::new()
            .endpoint("/admin/api/stats/top_domains")
            .ftl("top-domains (10)", data)
            .expect_json(
                json!({
                    "data": {
                        "top_domains": {
                            "example.com": 7,
                            "example.net": 3
                        },
                        "total_queries": 10
                    },
                    "errors": []
                })
            )
            .test();
    }

    #[test]
    fn test_top_domains_limit() {
        let mut data = Vec::new();
        encode::write_i32(&mut data, 10).unwrap();
        encode::write_str(&mut data, "example.com").unwrap();
        encode::write_i32(&mut data, 7).unwrap();
        write_eom(&mut data);

        TestBuilder::new()
            .endpoint("/admin/api/stats/top_domains?limit=1")
            .ftl("top-domains (1)", data)
            .expect_json(
                json!({
                    "data": {
                        "top_domains": {
                            "example.com": 7
                        },
                        "total_queries": 10
                    },
                    "errors": []
                })
            )
            .test();
    }
}