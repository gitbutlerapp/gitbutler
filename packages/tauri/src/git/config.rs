use super::{Error, Result};

pub struct Config {
    config: git2::Config,
}

impl From<git2::Config> for Config {
    fn from(config: git2::Config) -> Self {
        Self { config }
    }
}

impl From<Config> for git2::Config {
    fn from(v: Config) -> Self {
        v.config
    }
}

impl Config {
    pub fn set_str(&mut self, key: &str, value: &str) -> Result<()> {
        self.config.set_str(key, value).map_err(Into::into)
    }

    pub fn set_bool(&mut self, key: &str, value: bool) -> Result<()> {
        self.config.set_bool(key, value).map_err(Into::into)
    }

    pub fn set_multivar(&mut self, key: &str, regexp: &str, value: &str) -> Result<()> {
        self.config
            .set_multivar(key, regexp, value)
            .map_err(Into::into)
    }

    pub fn get_string(&self, key: &str) -> Result<Option<String>> {
        match self.config.get_string(key).map_err(Into::into) {
            Ok(value) => Ok(Some(value)),
            Err(Error::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_bool(&self, key: &str) -> Result<Option<bool>> {
        match self.config.get_bool(key).map_err(Into::into) {
            Ok(value) => Ok(Some(value)),
            Err(Error::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils;

    #[test]
    pub fn test_set_str() {
        let repo = test_utils::test_repository();
        let mut config = repo.config().unwrap();
        config.set_str("test.key", "test.value").unwrap();
        assert_eq!(
            config.get_string("test.key").unwrap().unwrap(),
            "test.value"
        );
    }

    #[test]
    pub fn test_set_bool() {
        let repo = test_utils::test_repository();
        let mut config = repo.config().unwrap();
        config.set_bool("test.key", true).unwrap();
        assert!(config.get_bool("test.key").unwrap().unwrap());
    }

    #[test]
    pub fn test_get_string_none() {
        let repo = test_utils::test_repository();
        let config = repo.config().unwrap();
        assert_eq!(config.get_string("test.key").unwrap(), None);
    }

    #[test]
    pub fn test_get_bool_none() {
        let repo = test_utils::test_repository();
        let config = repo.config().unwrap();
        assert_eq!(config.get_bool("test.key").unwrap(), None);
    }
}
