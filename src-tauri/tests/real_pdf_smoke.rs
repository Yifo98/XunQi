use std::{fs, path::PathBuf};

use tempfile::tempdir;
use xunqi_lib::{Application, ArticleExportMode};

#[test]
#[ignore = "requires a live public WeChat article and writes a visual QA artifact"]
fn exports_a_live_public_article_as_an_offline_pdf() {
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
    let output = app
        .export_article(task.id, output_dir, ArticleExportMode::Pdf)
        .expect("export PDF");
    assert!(PathBuf::from(output.destination).is_file());
}
