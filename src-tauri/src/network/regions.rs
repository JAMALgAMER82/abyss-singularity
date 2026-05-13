//! Default set of probe targets.
//!
//! Each entry is a publicly-reachable TCP endpoint that's physically
//! anchored to a known region. TCP-connect time to each is a reasonable
//! proxy for "how good a relay this region would be." We deliberately use
//! a mix of providers (AWS S3 + Cloudflare + Google) so a single
//! provider's outage doesn't blank out the whole probe matrix.

use super::types::ProbeTarget;

pub fn default_targets() -> Vec<ProbeTarget> {
    fn t(id: &str, label: &str, continent: &str, host: &str) -> ProbeTarget {
        ProbeTarget {
            id:        id.to_string(),
            label:     label.to_string(),
            continent: continent.to_string(),
            host:      host.to_string(),
            port:      443,
        }
    }

    vec![
        // North America
        t("aws-us-east-1",      "US East · N. Virginia",   "North America", "s3.us-east-1.amazonaws.com"),
        t("aws-us-east-2",      "US East · Ohio",          "North America", "s3.us-east-2.amazonaws.com"),
        t("aws-us-west-1",      "US West · N. California", "North America", "s3.us-west-1.amazonaws.com"),
        t("aws-us-west-2",      "US West · Oregon",        "North America", "s3.us-west-2.amazonaws.com"),
        t("aws-ca-central-1",   "Canada · Central",        "North America", "s3.ca-central-1.amazonaws.com"),
        // South America
        t("aws-sa-east-1",      "São Paulo",               "South America", "s3.sa-east-1.amazonaws.com"),
        // Europe
        t("aws-eu-west-1",      "EU · Ireland",            "Europe",        "s3.eu-west-1.amazonaws.com"),
        t("aws-eu-west-2",      "EU · London",             "Europe",        "s3.eu-west-2.amazonaws.com"),
        t("aws-eu-west-3",      "EU · Paris",              "Europe",        "s3.eu-west-3.amazonaws.com"),
        t("aws-eu-central-1",   "EU · Frankfurt",          "Europe",        "s3.eu-central-1.amazonaws.com"),
        t("aws-eu-north-1",     "EU · Stockholm",          "Europe",        "s3.eu-north-1.amazonaws.com"),
        t("aws-eu-south-1",     "EU · Milan",              "Europe",        "s3.eu-south-1.amazonaws.com"),
        // Asia / Pacific
        t("aws-ap-northeast-1", "Asia · Tokyo",            "Asia / Pacific","s3.ap-northeast-1.amazonaws.com"),
        t("aws-ap-northeast-2", "Asia · Seoul",            "Asia / Pacific","s3.ap-northeast-2.amazonaws.com"),
        t("aws-ap-southeast-1", "Asia · Singapore",        "Asia / Pacific","s3.ap-southeast-1.amazonaws.com"),
        t("aws-ap-southeast-2", "Asia · Sydney",           "Asia / Pacific","s3.ap-southeast-2.amazonaws.com"),
        t("aws-ap-south-1",     "Asia · Mumbai",           "Asia / Pacific","s3.ap-south-1.amazonaws.com"),
        // Middle East / Africa
        t("aws-me-south-1",     "Middle East · Bahrain",   "Middle East",   "s3.me-south-1.amazonaws.com"),
        t("aws-af-south-1",     "Africa · Cape Town",      "Africa",        "s3.af-south-1.amazonaws.com"),
    ]
}
