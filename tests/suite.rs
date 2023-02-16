use std::{error::Error, ffi::OsStr, fs::File, path::Path};

use jsonschema::{Compiler, Draft, Schemas, UrlLoader};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SUITE_DIR: &str = "tests/JSON-Schema-Test-Suite";
const TESTS_DIR: &str = "tests/JSON-Schema-Test-Suite/tests";
static SKIP: [&str; 5] = [
    "zeroTerminatedFloats.json", // only draft4: this behavior is changed in later drafts
    "float-overflow.json",
    "ecmascript-regex.json",
    "idn-hostname.json",
    "idn-email.json",
];

#[derive(Debug, Serialize, Deserialize)]
struct Group {
    description: String,
    schema: Value,
    tests: Vec<Test>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Test {
    description: String,
    data: Value,
    valid: bool,
}

#[test]
fn test_suite() -> Result<(), Box<dyn Error>> {
    run_dir("draft4", Draft::V4)?;
    run_dir("draft6", Draft::V6)?;
    run_dir("draft7", Draft::V7)?;
    run_dir("draft2019-09", Draft::V2019_09)?;
    run_dir("draft2020-12", Draft::V2020_12)?;
    Ok(())
}

fn run_dir(path: &str, draft: Draft) -> Result<(), Box<dyn Error>> {
    let path = Path::new(TESTS_DIR).join(path);
    for entry in path.read_dir()? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let entry_path = entry.path();
        let entry_path = entry_path.strip_prefix(TESTS_DIR)?.to_str().unwrap();
        if file_type.is_file() {
            if !SKIP.iter().any(|n| OsStr::new(n) == entry.file_name()) {
                run_file(entry_path, draft)?;
            }
        } else if file_type.is_dir() {
            run_dir(entry_path, draft)?;
        }
    }
    Ok(())
}

fn run_file(path: &str, draft: Draft) -> Result<(), Box<dyn Error>> {
    println!("FILE: {path}");
    let path = Path::new(TESTS_DIR).join(path);
    let optional = path.components().any(|comp| comp.as_os_str() == "optional");
    let file = File::open(path)?;

    let url = "http://testsuite.com/schema.json";
    let groups: Vec<Group> = serde_json::from_reader(file)?;
    for group in groups {
        println!("{}", group.description);
        let mut schemas = Schemas::default();
        let mut compiler = Compiler::default();
        compiler.set_default_draft(draft);
        if optional {
            compiler.enable_format_assertions();
            compiler.enable_content_assertions();
        }
        compiler.register_url_loader("http", Box::new(RemotesLoader));
        compiler.register_url_loader("https", Box::new(RemotesLoader));
        compiler.add_resource(url, group.schema)?;
        let sch_index = compiler.compile(&mut schemas, url.into())?;
        for test in group.tests {
            println!("    {}", test.description);
            let result = schemas.validate(&test.data, sch_index);
            if let Err(e) = &result {
                println!("        {e:#}");
            }
            assert_eq!(result.is_ok(), test.valid);
        }
    }
    Ok(())
}

struct RemotesLoader;
impl UrlLoader for RemotesLoader {
    fn load(&self, url: &url::Url) -> Result<Value, Box<dyn std::error::Error>> {
        // ensure that url has "localhost:1234"
        if url.as_str().starts_with("http://localhost:1234/") {
            let path = Path::new(SUITE_DIR).join("remotes").join(&url.path()[1..]);
            let file = File::open(path)?;
            let json: Value = serde_json::from_reader(file)?;
            return Ok(json);
        }

        // Meta-Schemas --
        let url = url.as_str();
        let meta = if let Some(suffix) = url.strip_prefix("http://json-schema.org/") {
            Some(suffix)
        } else if let Some(suffix) = url.strip_prefix("https://json-schema.org/") {
            Some(suffix)
        } else {
            None
        };
        if let Some(meta) = meta {
            let file = File::open(Path::new("src/metaschemas/").join(meta))?;
            let json: Value = serde_json::from_reader(file)?;
            return Ok(json);
        }

        Err("no internet")?
    }
}
