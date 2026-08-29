fn moved_between_threads<F: Send>(_: F) {}

#[test]
fn opening_a_store_is_something_a_host_can_move_between_threads() {
    moved_between_threads(gmr_store::sqlite::open_in_memory());
    moved_between_threads(gmr_store::sqlite::open(std::path::PathBuf::from("a.db")));
    moved_between_threads(gmr_store::sqlite::open_with(
        std::path::PathBuf::from("a.db"),
        gmr_store::sqlite::Pooling::default(),
    ));
}
