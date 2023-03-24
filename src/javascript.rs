use std::{fs::File, io::BufReader};

use camino::{Utf8Path, Utf8PathBuf};
use guppy::PackageId;
use miette::{Context, IntoDiagnostic};
use oro_common::{Manifest, Repository};

use crate::{PackageInfo, Result, SortedMap, WorkspaceInfo, WorkspaceKind};

pub fn get_project() -> Result<WorkspaceInfo> {
    let root = workspace_root().unwrap();
    let manifest_path = root.join("package.json");
    let manifest = load_manifest(&manifest_path)?;

    // For now this code is fairly naive and doesn't understand workspaces.
    // We assume the first package.json we find is "the root package" and
    // has the binary we care about.

    // Just assume ./node_modules is the target?
    let target_dir = root.join("node_modules");

    let root_auto_includes = crate::find_auto_includes(&root)?;

    // Not having a name is common to virtual manifests!
    let package_name = manifest
        .name
        .expect("your package doesn't have a name, is it a workspace? We don't support that yet.");
    let version = manifest.version.as_ref().map(|v| v.to_string());
    let authors = manifest
        .author
        .and_then(|a| match a {
            oro_common::PersonField::Str(s) => Some(vec![s]),
            // Not yet implemented!
            oro_common::PersonField::Obj(_) => None,
        })
        .unwrap_or_default();

    let repository_url = manifest.repository.and_then(|url| match url {
        // TODO: process this into a proper URL?
        //
        // It can be things like:
        //
        // * "npm/npm"
        // * "github:user/repo"
        // * "gist:11081aaa281"
        // * "bitbucket:user/repo"
        // * "gitlab:user/repo"
        //
        // Using the same syntax as https://docs.npmjs.com/cli/v7/commands/npm-install
        Repository::Str(repo) => Some(repo),
        Repository::Obj { url, .. } => url,
    });

    let mut info = PackageInfo {
        name: package_name.clone(),
        version,
        description: manifest.description,
        authors,
        license: manifest.license,
        // FIXME: is there any JS equivalent to this?
        publish: true,
        repository_url: repository_url.clone(),
        homepage_url: manifest.homepage,
        // FIXME: is there any JS equivalent to this?
        documentation_url: None,
        // FIXME: is there any JS equivalent to this?
        readme_file: None,
        // FIXME: is there any JS equivalent to this?
        license_files: vec![],
        // FIXME: is there any JS equivalent to this?
        changelog_file: None,
        // FIXME: don't just assume this is a binary?
        binaries: vec![package_name.clone()],
    };
    crate::merge_auto_includes(&mut info, &root_auto_includes);

    let mut package_info = SortedMap::new();
    package_info.insert(PackageId::new(package_name), info);

    Ok(WorkspaceInfo {
        kind: WorkspaceKind::Rust,
        target_dir,
        workspace_dir: root,
        package_info,
        manifest_path,
        repository_url,
        root_auto_includes,
    })
}

fn workspace_root() -> Option<Utf8PathBuf> {
    let current_dir = std::env::current_dir().expect("couldn't get current working dir!?");
    let start_dir =
        Utf8PathBuf::from_path_buf(current_dir).expect("current working dir isn't utf8!?");
    for path in start_dir.ancestors() {
        let pkg_json = path.join("package.json");
        if pkg_json.is_file() {
            return Some(path.to_owned());
        }
    }
    None
}

fn load_manifest(manifest_path: &Utf8Path) -> Result<Manifest> {
    let file = File::open(manifest_path)
        .into_diagnostic()
        .wrap_err("failed to read package.json")?;
    let reader = BufReader::new(file);
    let manifest: Manifest = serde_json::from_reader(reader)
        .into_diagnostic()
        .wrap_err("failed to parse package.json")?;
    Ok(manifest)
}
