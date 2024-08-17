//! Support for generic projects with cargo-dist build instructions

use axoasset::SourceFile;
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::{
    errors::AxoprojectError, PackageInfo, Result, Version, WorkspaceInfo, WorkspaceSearch,
};

#[derive(Deserialize)]
struct Manifest {
    workspace: Option<Workspace>,
    package: Option<Package>,
}

impl Manifest {
    fn workspace_members(&self) -> Option<Vec<String>> {
        if let Some(workspace) = &self.workspace {
            workspace.members.as_ref().map(|members| members.to_owned())
        } else {
            None
        }
    }
}

#[derive(Deserialize)]
struct Workspace {
    members: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Package {
    name: String,
    repository: Option<String>,
    homepage: Option<String>,
    documentation: Option<String>,
    description: Option<String>,
    readme: Option<Utf8PathBuf>,
    #[serde(default = "Vec::new")]
    authors: Vec<String>,
    binaries: Vec<String>,
    license: Option<String>,
    changelog: Option<Utf8PathBuf>,
    #[serde(default = "Vec::new")]
    license_files: Vec<Utf8PathBuf>,
    #[serde(default = "Vec::new")]
    cstaticlibs: Vec<String>,
    #[serde(default = "Vec::new")]
    cdylibs: Vec<String>,
    build_command: Vec<String>,
    version: Option<semver::Version>,
}

/// Try to find a generic workspace at the given path
///
/// See [`crate::get_workspaces`][] for the semantics.
pub fn get_workspace(start_dir: &Utf8Path, clamp_to_dir: Option<&Utf8Path>) -> WorkspaceSearch {
    let manifest_path = match crate::find_file("dist.toml", start_dir, clamp_to_dir) {
        Ok(path) => path,
        Err(e) => return WorkspaceSearch::Missing(e),
    };

    match workspace_from(&manifest_path) {
        Ok(info) => WorkspaceSearch::Found(info),
        Err(e) => WorkspaceSearch::Broken {
            manifest_path,
            cause: e,
        },
    }
}

fn workspace_from(manifest_path: &Utf8Path) -> Result<WorkspaceInfo> {
    let workspace_dir = manifest_path.parent().unwrap().to_path_buf();

    let manifest = load_root_dist_toml(manifest_path)?;
    // If this is a workspace, read its members and map those entries
    // to expected paths on disk
    let expected_paths = if let Some(members) = manifest.workspace_members() {
        members
            .iter()
            .map(|name| workspace_dir.join(name))
            .map(Utf8PathBuf::from)
            .collect()
    // If this *isn't* a workspace, the root is the only app
    } else if manifest.package.is_some() {
        vec![workspace_dir.to_path_buf()]
    } else {
        return Err(AxoprojectError::DistTomlMalformedError {
            path: manifest_path.to_path_buf(),
        });
    };

    workspace_info(manifest_path, &workspace_dir, &expected_paths)
}

fn package_info(manifest_root: &Utf8PathBuf) -> Result<PackageInfo> {
    let manifest_path = manifest_root.join("dist.toml");
    let manifest = load_root_dist_toml(&manifest_path)?;

    let package = if let Some(package) = manifest.package {
        package
    } else {
        return Err(AxoprojectError::PackageMissingError {
            path: manifest_path,
        });
    };
    let version = package.version.map(Version::Generic);

    Ok(PackageInfo {
        manifest_path: manifest_path.clone(),
        package_root: manifest_path.clone(),
        name: package.name,
        version,
        description: package.description,
        authors: package.authors,
        license: package.license,
        publish: true,
        keywords: None,
        repository_url: package.repository.clone(),
        homepage_url: package.homepage,
        documentation_url: package.documentation,
        readme_file: package.readme,
        license_files: package.license_files,
        changelog_file: package.changelog,
        binaries: package.binaries,
        cstaticlibs: package.cstaticlibs,
        cdylibs: package.cdylibs,
        #[cfg(feature = "cargo-projects")]
        cargo_metadata_table: None,
        #[cfg(feature = "cargo-projects")]
        cargo_package_id: None,
        build_command: Some(package.build_command),
    })
}

fn workspace_info(
    manifest_path: &Utf8Path,
    workspace_dir: &Utf8PathBuf,
    expected_paths: &[Utf8PathBuf],
) -> Result<WorkspaceInfo> {
    let root_auto_includes = crate::find_auto_includes(workspace_dir)?;

    let package_info = expected_paths
        .iter()
        .map(package_info)
        .collect::<Result<Vec<PackageInfo>>>()?;

    let repository_url = package_info
        .first()
        .map(|p| p.repository_url.to_owned())
        .unwrap_or(None);

    Ok(WorkspaceInfo {
        kind: crate::WorkspaceKind::Generic,
        target_dir: workspace_dir.join("target"),
        workspace_dir: workspace_dir.to_owned(),
        package_info,
        manifest_path: manifest_path.to_owned(),
        repository_url,
        root_auto_includes,
        warnings: vec![],
        #[cfg(feature = "cargo-projects")]
        cargo_metadata_table: None,
        #[cfg(feature = "cargo-projects")]
        cargo_profiles: crate::rust::CargoProfiles::new(),
    })
}

/// Load the root workspace toml
fn load_root_dist_toml(manifest_path: &Utf8Path) -> Result<Manifest> {
    let manifest_src = SourceFile::load_local(manifest_path)?;
    let manifest = manifest_src.deserialize_toml()?;
    Ok(manifest)
}
