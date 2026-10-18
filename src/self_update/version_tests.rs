use super::*;

#[test]
fn parses_with_and_without_the_v_prefix() {
    assert_eq!("v0.10.2".parse::<Version>(), "0.10.2".parse::<Version>());
    assert_eq!(
        "0.10.2".parse::<Version>().map(|v| v.to_string()),
        Ok("0.10.2".to_owned())
    );
}

#[test]
fn rejects_anything_that_is_not_three_numbers() {
    for bad in [
        "", "1", "1.2", "1.2.3.4", "1.2.x", "latest", "v", "1..3", "-1.2.3", "vv1.2.3", "+1.2.3",
        "1.+2.3",
    ] {
        assert!(bad.parse::<Version>().is_err(), "{bad} should not parse");
    }
}

#[test]
fn orders_numerically_not_as_text() {
    let low: Version = "0.9.10".parse().expect("valid test fixture");
    let high: Version = "0.10.2".parse().expect("valid test fixture");
    assert!(low < high);
    assert!("1.0.0".parse::<Version>().expect("valid test fixture") > high);
}

#[test]
fn tag_adds_the_v_prefix() {
    assert_eq!(
        "0.10.2"
            .parse::<Version>()
            .expect("valid test fixture")
            .tag(),
        "v0.10.2"
    );
}

#[test]
fn this_binary_knows_its_own_version() {
    assert!(Version::current().is_some());
}

#[test]
fn every_shipped_platform_maps_to_a_release_asset() {
    let pairs = [
        ("linux", "x86_64"),
        ("linux", "aarch64"),
        ("macos", "x86_64"),
        ("macos", "aarch64"),
    ];
    for (os, arch) in pairs {
        let target = Target::from_parts(os, arch).expect("valid test fixture");
        assert!(target.asset().starts_with("opman-") && target.asset().ends_with(".tar.gz"));
    }
}

#[test]
fn unshipped_platforms_are_refused() {
    assert_eq!(Target::from_parts("windows", "x86_64"), None);
    assert_eq!(Target::from_parts("linux", "riscv64"), None);
}
