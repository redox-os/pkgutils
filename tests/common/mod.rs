extern crate pkgutils;

pub fn db_location() -> String {
    format!("{}/tests/test_db/", env!("CARGO_MANIFEST_DIR"))
}

pub fn get_db() -> pkgutils::Database {
    let path = db_location();
    pkgutils::Database::open(format!("{}/pkg", path), format!("{}/etc/pkg.d/pkglist", path))
}
