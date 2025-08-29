mod create_zip_file_from_dir {
    use but_feedback::create_zip_file_from_dir;
    use std::{fs::File, io::Write};
    use tempfile::tempdir;

    #[test]
    fn zip_dir() -> anyhow::Result<()> {
        let tmp_dir = tempdir()?;
        let tmp_dir_path = tmp_dir.path();
        let file_path = tmp_dir_path.join("test.txt");
        let mut file = File::create(file_path)?;
        file.write_all(b"test")?;

        let out_dir = tempdir()?;
        let zip_file_path =
            create_zip_file_from_dir(tmp_dir.path(), out_dir.path().join("out.zip"))?;
        assert!(zip_file_path.exists());
        Ok(())
    }

    #[test]
    fn zip_file_fails() -> anyhow::Result<()> {
        let tmp_dir = tempdir()?;
        let tmp_dir_path = tmp_dir.path();
        let file_path = tmp_dir_path.join("test.txt");
        let mut file = File::create(&file_path)?;
        file.write_all(b"test")?;

        let out_dir = tempdir()?;
        let err = create_zip_file_from_dir(file_path, out_dir.path().join("out.zip")).unwrap_err();
        assert!(err.to_string().ends_with("s not a directory"));
        Ok(())
    }
}

mod create_zip_file_from_stream {
    use but_feedback::create_zip_file_from_content;
    use tempfile::tempdir;

    #[test]
    fn zip_file() -> anyhow::Result<()> {
        let out_dir = tempdir()?;
        let zip_file_path = create_zip_file_from_content(
            "the content of the file in the archive",
            "the-file-in-the-archive.foo",
            out_dir.path().join("out.zip"),
        )?;
        assert!(zip_file_path.exists());
        Ok(())
    }
}
