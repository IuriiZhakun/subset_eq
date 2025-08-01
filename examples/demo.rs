use subset_eq::subset_eq;

#[derive(Debug, Clone, PartialEq, Eq)]
#[subset_eq(ignore(updated_at, cache_token), method = "eq_ignoring_meta")]
struct Item {
    id: u64,
    name: String,
    updated_at: i64,
    cache_token: String,
}

fn main() {
    let a = Item {
        id: 1,
        name: "Alice".into(),
        updated_at: 100,
        cache_token: "tok".into(),
    };
    let mut b = a.clone();
    b.updated_at = 200;
    b.cache_token = "other".into();

    println!("Full equality: {}", a == b);
    println!("Subset equality ignoring metadata: {}", a.eq_ignoring_meta(&b));
}

