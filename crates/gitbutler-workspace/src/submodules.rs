pub fn has_submodules_configured(repo: &git2::Repository) -> bool {
    if repo
        .workdir()
        .is_some_and(|workdir| workdir.join(".gitmodules").exists())
    {
        return true;
    }

    let Ok(config) = repo.config() else {
        return false;
    };

    let modules_dir_has_entries = repo
        .path()
        .join("modules")
        .read_dir()
        .map(|mut it| it.next().is_some())
        .unwrap_or(false);

    let Ok(mut entries) = config.entries(Some("submodule\\..*\\.url")) else {
        return modules_dir_has_entries;
    };

    entries.next().transpose().ok().flatten().is_some() || modules_dir_has_entries
}
