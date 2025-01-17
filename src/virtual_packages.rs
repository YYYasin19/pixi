use anyhow::bail;
use rattler_conda_types::{GenericVirtualPackage, Platform, Version};
use rattler_virtual_packages::{Archspec, Cuda, LibC, Linux, Osx, VirtualPackage};
use serde::Deserialize;
use std::collections::HashSet;
use std::str::FromStr;

/// The supported system requirements that can be defined in the configuration.
#[derive(Debug, Deserialize)]
pub struct SystemRequirements {
    windows: Option<bool>,
    unix: Option<bool>,
    linux: Option<Linux>,
    osx: Option<String>,
    libc: Option<LibC>,
    cuda: Option<String>,
    archspec: Option<Archspec>,
}

impl From<SystemRequirements> for Vec<VirtualPackage> {
    fn from(requirements: SystemRequirements) -> Vec<VirtualPackage> {
        let mut packages = Vec::new();

        if let Some(true) = requirements.windows {
            packages.push(VirtualPackage::Win);
        }

        if let Some(true) = requirements.unix {
            packages.push(VirtualPackage::Unix);
        }

        if let Some(linux_config) = requirements.linux {
            packages.push(VirtualPackage::Linux(linux_config));
        }

        if let Some(version) = requirements.osx {
            packages.push(VirtualPackage::Osx(Osx {
                version: Version::from_str(version.as_str()).unwrap(),
            }));
        }

        if let Some(libc_config) = requirements.libc {
            packages.push(VirtualPackage::LibC(libc_config));
        }

        if let Some(version) = requirements.cuda {
            packages.push(VirtualPackage::Cuda(Cuda {
                version: Version::from_str(version.as_str()).unwrap(),
            }));
        }

        if let Some(archspec_config) = requirements.archspec {
            packages.push(VirtualPackage::Archspec(archspec_config));
        }

        packages
    }
}

/// Returns a reasonable modern set of virtual packages that should be safe enough to assume.
/// At the time of writing, this is in sync with the conda-lock set of minimal virtual packages.
/// <https://github.com/conda/conda-lock/blob/3d36688278ebf4f65281de0846701d61d6017ed2/conda_lock/virtual_package.py#L175>
pub fn get_minimal_virtual_packages(platform: Platform) -> Vec<VirtualPackage> {
    // TODO: How to add a default cuda requirements
    let mut virtual_packages: Vec<VirtualPackage> = vec![];

    // Match high level platforms
    if platform.is_unix() {
        virtual_packages.push(VirtualPackage::Unix);
    }
    if platform.is_linux() {
        virtual_packages.push(VirtualPackage::Linux(Linux {
            version: "5.10".parse().unwrap(),
        }));
        virtual_packages.push(VirtualPackage::LibC(LibC {
            family: "glibc".parse().unwrap(),
            version: "2.17".parse().unwrap(),
        }));
    }
    if platform.is_windows() {
        virtual_packages.push(VirtualPackage::Win);
    }

    if let Some(archspec) = Archspec::from_platform(platform) {
        virtual_packages.push(archspec.into())
    }

    // Add platform specific packages
    match platform {
        Platform::OsxArm64 => {
            virtual_packages.push(VirtualPackage::Osx(Osx {
                version: "11.0".parse().unwrap(),
            }));
        }
        Platform::Osx64 => {
            virtual_packages.push(VirtualPackage::Osx(Osx {
                version: "10.15".parse().unwrap(),
            }));
        }
        _ => {}
    }
    virtual_packages
}

/// Verifies if the current platform satisfies the minimal virtual package requirements.
pub fn verify_current_platform_has_required_virtual_packages(
    custom_system_requirements: &[GenericVirtualPackage],
) -> Result<(), anyhow::Error> {
    let local_vpkgs = VirtualPackage::current().map(|vpkgs| {
        vpkgs
            .iter()
            .map(|vpkg| GenericVirtualPackage::from(vpkg.clone()))
            .collect::<Vec<_>>()
    })?;

    // The required virtual packages for the current system
    let required_vpkgs: Vec<GenericVirtualPackage> =
        get_minimal_virtual_packages(Platform::current())
            .iter()
            .map(|vpkg| GenericVirtualPackage::from(vpkg.clone()))
            .chain(custom_system_requirements.to_owned())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

    // Check for every local minimum package if it is available and on the correct version.
    for req_pkg in required_vpkgs {
        if let Some(local_vpkg) = local_vpkgs
            .iter()
            .find(|&pkg| pkg.name == req_pkg.name && pkg.build_string == req_pkg.build_string)
        {
            if req_pkg.version > local_vpkg.version {
                bail!("The platform you are running on does not contain the minimal version ({}) of the virtual package {}, overwrite it or use newer system for this package.", req_pkg.version, req_pkg.name)
            }
        } else {
            bail!("The platform you are running on should at least have the virtual package: {} on version: {} and build_string: {}", req_pkg.name, req_pkg.version, req_pkg.build_string)
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::virtual_packages::get_minimal_virtual_packages;
    use insta::assert_debug_snapshot;
    use rattler_conda_types::Platform;

    // Regression test on the virtual packages so there is not accidental changes
    #[test]
    fn test_get_minimal_virtual_packages() {
        let platforms = vec![
            Platform::NoArch,
            Platform::Linux64,
            Platform::LinuxAarch64,
            Platform::LinuxPpc64le,
            Platform::Osx64,
            Platform::OsxArm64,
            Platform::Win64,
        ];

        for platform in platforms {
            let packages = get_minimal_virtual_packages(platform);
            let snapshot_name = format!("test_get_minimal_virtual_packages.{}", platform);
            assert_debug_snapshot!(snapshot_name, packages);
        }
    }
}
