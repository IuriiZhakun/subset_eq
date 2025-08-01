use subset_eq::subset_eq;

#[derive(Debug, Clone, PartialEq, Eq)]
#[subset_eq(ignore(updated_at, cache_token), method = "eq_ignoring_meta")]
struct Item {
    id: u64,
    name: String,
    updated_at: i64,
    cache_token: String,
}

#[test]
fn full_eq_detects_change() {
    let a = Item {
        id: 1,
        name: "foo".into(),
        updated_at: 100,
        cache_token: "tok".into(),
    };
    let mut b = a.clone();
    b.name = "bar".into();
    assert_ne!(a, b);
}

#[test]
fn subset_eq_ignores() {
    let a = Item {
        id: 2,
        name: "foo".into(),
        updated_at: 100,
        cache_token: "tok".into(),
    };
    let mut b = a.clone();
    b.updated_at = 999;
    b.cache_token = "xyz".into();
    assert!(a.eq_ignoring_meta(&b));
}

#[test]
fn subset_eq_fails_on_real_diff() {
    let a = Item {
        id: 5,
        name: "foo".into(),
        updated_at: 0,
        cache_token: "tok".into(),
    };
    let different = Item {
        id: 6,
        name: "foo".into(),
        updated_at: 0,
        cache_token: "tok".into(),
    };
    assert!(!a.eq_ignoring_meta(&different));
}
