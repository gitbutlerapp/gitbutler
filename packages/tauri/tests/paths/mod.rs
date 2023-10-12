use gitbutler::paths::DataDir;

use crate::common::temp_dir;

pub fn data_dir() -> DataDir {
    DataDir::from(temp_dir())
}
