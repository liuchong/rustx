use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct ScriptInput {
    pub absolute_path: PathBuf,
    pub raw_contents: String,
    pub manifest_block: Option<String>,
    pub rust_source: String,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ParsedScript {
    pub manifest_block: Option<String>,
    pub rust_source: String,
}

pub fn load(path: &Path) -> Result<ScriptInput, String> {
    let absolute_path = path
        .canonicalize()
        .map_err(|err| format!("script not found: {}: {}", path.display(), err))?;
    let raw_contents = fs::read_to_string(&absolute_path).map_err(|err| {
        format!(
            "failed to read script: {}: {}",
            absolute_path.display(),
            err
        )
    })?;
    let parsed = parse_contents(&raw_contents)?;

    Ok(ScriptInput {
        absolute_path,
        raw_contents,
        manifest_block: parsed.manifest_block,
        rust_source: parsed.rust_source,
    })
}

pub fn parse_contents(contents: &str) -> Result<ParsedScript, String> {
    let body_start = if contents.starts_with("#!") {
        contents
            .find('\n')
            .map(|pos| pos + 1)
            .unwrap_or(contents.len())
    } else {
        0
    };
    let body = &contents[body_start..];

    let spans = line_spans(body);
    let manifest_start = find_manifest_start(&spans);

    let Some((start, marker_end)) = manifest_start else {
        return Ok(ParsedScript {
            manifest_block: None,
            rust_source: body.to_string(),
        });
    };

    for (line_start, line_end, line) in spans {
        if line_start < marker_end {
            continue;
        }

        if marker(line) == "---" {
            let manifest_block = body[marker_end..line_start].to_string();
            let mut rust_source = String::with_capacity(body.len());
            rust_source.push_str(&body[..start]);
            rust_source.push_str(&body[line_end..]);

            return Ok(ParsedScript {
                manifest_block: Some(manifest_block),
                rust_source,
            });
        }
    }

    Err("embedded cargo manifest is missing closing --- marker".to_string())
}

fn find_manifest_start(spans: &[(usize, usize, &str)]) -> Option<(usize, usize)> {
    for (start, end, line) in spans {
        let marker = marker(line);
        if marker.is_empty() || marker.starts_with("//") {
            continue;
        }

        return (marker == "---cargo").then_some((*start, *end));
    }

    None
}

fn marker(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n']).trim()
}

fn line_spans(input: &str) -> Vec<(usize, usize, &str)> {
    let mut spans = Vec::new();
    let mut start = 0;

    for line in input.split_inclusive('\n') {
        let end = start + line.len();
        spans.push((start, end, line));
        start = end;
    }

    if start < input.len() {
        spans.push((start, input.len(), &input[start..]));
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::parse_contents;

    #[test]
    fn strips_shebang() {
        let parsed = parse_contents("#!/usr/bin/env rustx\nfn main() {}\n").unwrap();

        assert_eq!(parsed.manifest_block, None);
        assert_eq!(parsed.rust_source, "fn main() {}\n");
    }

    #[test]
    fn extracts_embedded_manifest() {
        let parsed =
            parse_contents("#!/usr/bin/env rustx\n---cargo\n[dependencies]\n---\nfn main() {}\n")
                .unwrap();

        assert_eq!(parsed.manifest_block, Some("[dependencies]\n".to_string()));
        assert_eq!(parsed.rust_source, "fn main() {}\n");
    }

    #[test]
    fn keeps_comments_before_manifest_in_source() {
        let parsed = parse_contents(
            "// license\n---cargo\n[package]\nedition = \"2021\"\n---\nfn main() {}\n",
        )
        .unwrap();

        assert_eq!(
            parsed.manifest_block,
            Some("[package]\nedition = \"2021\"\n".to_string())
        );
        assert_eq!(parsed.rust_source, "// license\nfn main() {}\n");
    }

    #[test]
    fn rejects_unclosed_manifest() {
        let error = parse_contents("---cargo\n[dependencies]\n").unwrap_err();

        assert!(error.contains("missing closing"));
    }

    #[test]
    fn ignores_manifest_marker_after_rust_code_starts() {
        let input = "fn main() {}\n---cargo\n[dependencies]\n---\n";
        let parsed = parse_contents(input).unwrap();

        assert_eq!(parsed.manifest_block, None);
        assert_eq!(parsed.rust_source, input);
    }

    #[test]
    fn handles_crlf_shebang_and_manifest_markers() {
        let parsed = parse_contents(
            "#!/usr/bin/env rustx\r\n---cargo\r\n[dependencies]\r\n---\r\nfn main() {}\r\n",
        )
        .unwrap();

        assert_eq!(
            parsed.manifest_block,
            Some("[dependencies]\r\n".to_string())
        );
        assert_eq!(parsed.rust_source, "fn main() {}\r\n");
    }
}
