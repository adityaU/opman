use std::path::PathBuf;

use nucleo::{Injector, Utf32String};

pub(super) fn walk_directories(
    root: PathBuf,
    injector: Injector<String>,
    existing_projects: Vec<String>,
) {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let home_str = home.to_string_lossy().to_string();

    let mut seen = std::collections::HashSet::new();

    // Inject existing projects first so they appear immediately
    for proj_path in &existing_projects {
        seen.insert(proj_path.clone());
        let display = if proj_path.starts_with(&home_str) {
            format!("~{} ★", &proj_path[home_str.len()..])
        } else {
            format!("{} ★", proj_path)
        };
        let data = proj_path.clone();
        let _ = injector.push(data, |_, cols| {
            cols[0] = Utf32String::from(display.as_str());
        });
    }

    let walker = ignore::WalkBuilder::new(&root)
        .standard_filters(false)
        .follow_links(true)
        .max_depth(Some(5))
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            if entry.depth() > 0 && name.starts_with('.') {
                return false;
            }
            match name.as_ref() {
                "node_modules" | "target" | "__pycache__" | ".git" | "vendor" | "dist"
                | "build" | ".cache" | "Library" | "Pictures" | "Music" | "Movies" => false,
                _ => true,
            }
        })
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }

        if entry.depth() == 0 {
            continue;
        }

        let path = entry.path().to_string_lossy().to_string();

        // Skip paths already injected as existing projects
        if seen.contains(&path) {
            continue;
        }

        let display = if path.starts_with(&home_str) {
            format!("~{}", &path[home_str.len()..])
        } else {
            path.clone()
        };

        let _ = injector.push(path, |_, cols| {
            cols[0] = Utf32String::from(display.as_str());
        });
    }
}
