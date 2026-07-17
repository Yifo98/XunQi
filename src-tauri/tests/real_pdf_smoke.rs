use std::{fs, path::PathBuf};

use tempfile::tempdir;
use xunqi_lib::{Application, ArticleExportMode};

#[test]
#[ignore = "requires a live public WeChat article and writes visual QA artifacts"]
fn exports_a_live_public_article_as_offline_pdf_and_markdown() {
    let article_url = std::env::var("XUNQI_REAL_ARTICLE_URL").expect("article URL");
    let output_dir = PathBuf::from(std::env::var("XUNQI_REAL_OUTPUT_DIR").expect("output dir"));
    fs::create_dir_all(&output_dir).expect("create output dir");
    let temp = tempdir().expect("temporary state");
    let app = Application::open(temp.path().join("xunqi.db")).expect("open application");
    let task = app
        .submit_links(&article_url)
        .expect("submit article")
        .tasks
        .remove(0);
    let detail = app.process_task(task.id).expect("read public article");
    let snapshot = detail.article.expect("article snapshot");
    assert!(!snapshot.body_html.is_empty());
    assert!(!snapshot.image_urls.is_empty());
    let pdf_output = app
        .export_article(task.id, output_dir.clone(), ArticleExportMode::Pdf)
        .expect("export PDF");
    assert!(PathBuf::from(pdf_output.destination).is_file());
    let markdown_output = app
        .export_article(task.id, output_dir, ArticleExportMode::Markdown)
        .expect("export Markdown");
    let markdown_directory = PathBuf::from(markdown_output.destination);
    assert!(markdown_directory.is_dir());
    assert!(markdown_directory.join("文章.md").is_file());
    assert!(markdown_directory.join("images").is_dir());
}
