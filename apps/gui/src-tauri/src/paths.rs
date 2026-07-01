use std::path::PathBuf;

pub fn expand_user_path(path: PathBuf) -> Result<PathBuf, String> {
    expand_user_path_with_home(path, home_dir())
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .or_else(|| {
                let drive = std::env::var_os("HOMEDRIVE")?;
                let path = std::env::var_os("HOMEPATH")?;
                let mut home = PathBuf::from(drive);
                home.push(path);
                Some(home)
            })
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

fn expand_user_path_with_home(path: PathBuf, home: Option<PathBuf>) -> Result<PathBuf, String> {
    let Some(raw) = path.to_str() else {
        return Ok(path);
    };
    let Some(rest) = (raw == "~")
        .then_some("")
        .or_else(|| raw.strip_prefix("~/"))
        .or_else(|| raw.strip_prefix("~\\"))
    else {
        return Ok(path);
    };
    let home = home.ok_or_else(|| {
        format!(
            "cannot expand '{}' because the home directory is unavailable",
            raw
        )
    })?;
    Ok(if rest.is_empty() { home } else { home.join(rest) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_home_prefix_is_expanded_before_path_use() {
        let expanded = expand_user_path_with_home(
            PathBuf::from("~/Downloads/augmented-gaussian"),
            Some(PathBuf::from("/home/tester")),
        )
        .unwrap();

        assert_eq!(
            expanded,
            PathBuf::from("/home/tester/Downloads/augmented-gaussian")
        );
        assert_eq!(
            expand_user_path_with_home(
                PathBuf::from("target/gui-output"),
                Some(PathBuf::from("/home/tester"))
            )
            .unwrap(),
            PathBuf::from("target/gui-output")
        );
    }
}
