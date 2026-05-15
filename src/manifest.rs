#[derive(Debug, Eq, PartialEq)]
pub struct GeneratedManifest {
    pub contents: String,
    pub package_name: String,
}

#[derive(Debug, Default)]
struct PackageScan {
    has_package: bool,
    has_name: bool,
    has_version: bool,
    has_edition: bool,
    package_name: Option<String>,
}

pub fn generate(
    manifest_block: Option<&str>,
    default_package_name: &str,
) -> Result<GeneratedManifest, String> {
    match manifest_block {
        Some(manifest) => generate_from_embedded(manifest, default_package_name),
        None => Ok(GeneratedManifest {
            contents: default_manifest(default_package_name),
            package_name: default_package_name.to_string(),
        }),
    }
}

fn generate_from_embedded(
    manifest: &str,
    default_package_name: &str,
) -> Result<GeneratedManifest, String> {
    let scan = scan_package(manifest);
    let package_name = scan
        .package_name
        .clone()
        .unwrap_or_else(|| default_package_name.to_string());

    let contents = if scan.has_package {
        fill_package_fields(manifest, default_package_name, &scan)
    } else {
        let mut contents = package_table(default_package_name);
        contents.push('\n');
        contents.push_str(manifest);
        ensure_trailing_newline(&mut contents);
        contents
    };

    Ok(GeneratedManifest {
        contents,
        package_name,
    })
}

fn default_manifest(package_name: &str) -> String {
    let mut contents = package_table(package_name);
    contents.push_str("\n[dependencies]\n");
    contents
}

fn package_table(package_name: &str) -> String {
    format!(
        "[package]\nname = \"{}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
        package_name
    )
}

fn fill_package_fields(manifest: &str, default_package_name: &str, scan: &PackageScan) -> String {
    let mut contents = String::with_capacity(manifest.len() + 96);
    let mut filled = false;

    for line in manifest.split_inclusive('\n') {
        contents.push_str(line);

        if !filled && is_package_table(line) {
            if !line.ends_with('\n') {
                contents.push('\n');
            }
            push_missing_package_fields(&mut contents, default_package_name, scan);
            filled = true;
        }
    }

    if !filled && !manifest.is_empty() {
        contents.push('\n');
        push_missing_package_fields(&mut contents, default_package_name, scan);
    }

    ensure_trailing_newline(&mut contents);
    contents
}

fn push_missing_package_fields(
    contents: &mut String,
    default_package_name: &str,
    scan: &PackageScan,
) {
    if !scan.has_name {
        contents.push_str(&format!("name = \"{}\"\n", default_package_name));
    }
    if !scan.has_version {
        contents.push_str("version = \"0.0.0\"\n");
    }
    if !scan.has_edition {
        contents.push_str("edition = \"2021\"\n");
    }
}

fn scan_package(manifest: &str) -> PackageScan {
    let mut scan = PackageScan::default();
    let mut in_package = false;

    for line in manifest.lines() {
        let logical = strip_comment(line).trim();

        if logical.starts_with('[') {
            in_package = logical == "[package]";
            scan.has_package |= in_package;
            continue;
        }

        if !in_package {
            continue;
        }

        let Some((key, value)) = logical.split_once('=') else {
            continue;
        };
        let key = key.trim();

        match key {
            "name" => {
                scan.has_name = true;
                scan.package_name = parse_string(value.trim());
            }
            "version" => scan.has_version = true,
            "edition" => scan.has_edition = true,
            _ => {}
        }
    }

    scan
}

fn is_package_table(line: &str) -> bool {
    strip_comment(line).trim() == "[package]"
}

fn strip_comment(line: &str) -> &str {
    line.split('#').next().unwrap_or(line)
}

fn parse_string(value: &str) -> Option<String> {
    let mut chars = value.chars();
    let quote = chars.next()?;

    if quote != '"' && quote != '\'' {
        return None;
    }

    let rest = chars.as_str();
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn ensure_trailing_newline(contents: &mut String) {
    if !contents.ends_with('\n') {
        contents.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::generate;

    #[test]
    fn generates_default_manifest() {
        let manifest = generate(None, "rustx_script_abc").unwrap();

        assert_eq!(manifest.package_name, "rustx_script_abc");
        assert!(manifest.contents.contains("name = \"rustx_script_abc\""));
        assert!(manifest.contents.contains("[dependencies]"));
    }

    #[test]
    fn prepends_package_table_when_missing() {
        let manifest = generate(Some("[dependencies]\n"), "rustx_script_abc").unwrap();

        assert!(manifest.contents.starts_with("[package]\n"));
        assert!(manifest.contents.contains("name = \"rustx_script_abc\""));
        assert!(manifest.contents.contains("[dependencies]\n"));
    }

    #[test]
    fn fills_missing_package_fields() {
        let manifest =
            generate(Some("[package]\nedition = \"2021\"\n"), "rustx_script_abc").unwrap();

        assert_eq!(manifest.package_name, "rustx_script_abc");
        assert!(manifest.contents.contains("name = \"rustx_script_abc\""));
        assert!(manifest.contents.contains("version = \"0.0.0\""));
        assert_eq!(manifest.contents.matches("edition = \"2021\"").count(), 1);
    }

    #[test]
    fn preserves_package_name() {
        let manifest = generate(Some("[package]\nname = \"tool\"\n"), "rustx_script_abc").unwrap();

        assert_eq!(manifest.package_name, "tool");
        assert!(manifest.contents.contains("name = \"tool\""));
        assert!(!manifest.contents.contains("name = \"rustx_script_abc\""));
    }
}
