use gitbutler_testsupport::test_repository;

#[test]
pub fn set_str() {
    let (repo, _tmp) = test_repository();
    let mut config = repo.config().unwrap();
    config.set_str("test.key", "test.value").unwrap();
    assert_eq!(
        config.get_string("test.key").unwrap().unwrap(),
        "test.value"
    );
}

#[test]
pub fn set_bool() {
    let (repo, _tmp) = test_repository();
    let mut config = repo.config().unwrap();
    config.set_bool("test.key", true).unwrap();
    assert!(config.get_bool("test.key").unwrap().unwrap());
}

#[test]
pub fn get_string_none() {
    let (repo, _tmp) = test_repository();
    let config = repo.config().unwrap();
    assert_eq!(config.get_string("test.key").unwrap(), None);
}

#[test]
pub fn get_bool_none() {
    let (repo, _tmp) = test_repository();
    let config = repo.config().unwrap();
    assert_eq!(config.get_bool("test.key").unwrap(), None);
}
