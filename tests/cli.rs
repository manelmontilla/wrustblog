use assert_cmd::prelude::*;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::{fs, thread};

use ureq;
use wruster::test_utils::get_free_port;

#[test]
fn serves_home_page() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("wrustblog")?;
    let blog_path = test_blog_dir();
    let (content, templates) = blog_path.clone();
    let port = get_free_port();
    let addr = format!("0.0.0.0:{}", port.to_string());
    cmd.arg("serve")
        .arg(templates)
        .arg(content)
        .arg(addr)
        .stdout(Stdio::piped());
    let mut process = cmd.spawn().unwrap();

    wait_for_line(&mut process, "listening on");

    // Make the test request.
    let blog_url = format!("http://localhost:{}/", port);
    let home_page_result = ureq::get(&blog_url).call()?;

    let post_url = format!("http://localhost:{}/posts/post-1", port);
    let post_page_result = ureq::get(&post_url).call().unwrap();

    assert_eq!(
        read_test_file("expected-home.html"),
        home_page_result.into_string().unwrap()
    );

    assert_eq!(
        read_test_file("expected-post.html"),
        post_page_result.into_string().unwrap()
    );

    process.kill().unwrap();

    Ok(())
}

fn read_test_file(file_path: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push(file_path);
    let mut f = fs::File::open(path).unwrap();
    let mut contents = String::new();
    f.read_to_string(&mut contents).unwrap();
    contents
}

fn wait_for_line(p: &mut Child, line: &str) {
    thread::scope(|_| {
        let stdout = p.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);
        let mut exit = false;
        while !exit {
            let mut read_line = String::new();
            reader.read_line(&mut read_line).unwrap();
            if read_line.contains(line) {
                exit = true
            }
        }
    })
}

fn test_blog_dir() -> (String, String) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/blog");
    let mut content = path.clone();
    content.push("content");
    let mut templates = path.clone();
    templates.push("templates");
    let content: String = content.to_str().unwrap().try_into().unwrap();
    let templates: String = templates.to_str().unwrap().try_into().unwrap();
    (content, templates)
}
