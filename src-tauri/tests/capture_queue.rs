use rusqlite::Connection;
use tempfile::tempdir;
use xunqi_lib::{AppError, Application, CaptureKind, CaptureStatus};

#[test]
fn shared_wechat_links_are_classified_deduplicated_and_persisted_as_active_tasks() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("v2").join("xunqi.db");
    let app = Application::open(&database).expect("open application");

    let submitted = app
        .submit_links(
            "文章 https://mp.weixin.qq.com/s/article-one ，视频 https://weixin.qq.com/sph/video-one",
        )
        .expect("submit links");
    assert_eq!(submitted.tasks.len(), 2);
    assert!(submitted
        .tasks
        .iter()
        .any(|task| task.kind == CaptureKind::Article));
    assert!(submitted
        .tasks
        .iter()
        .any(|task| task.kind == CaptureKind::Video));
    assert!(submitted
        .tasks
        .iter()
        .all(|task| task.status == CaptureStatus::Queued));

    let duplicate = app
        .submit_links("https://mp.weixin.qq.com/s/article-one")
        .expect("deduplicate link");
    assert_eq!(duplicate.tasks.len(), 1);
    assert_eq!(duplicate.duplicate_count, 1);
    assert_eq!(app.list_tasks().expect("list tasks").len(), 2);

    drop(app);
    let reopened = Application::open(&database).expect("reopen application");
    assert_eq!(reopened.list_tasks().expect("persisted tasks").len(), 2);

    assert!(matches!(
        reopened.submit_links("https://example.com/not-wechat"),
        Err(AppError::Validation(message)) if message.contains("微信公众号或视频号")
    ));
}

#[test]
fn v2_data_can_live_beside_a_legacy_database_without_loading_old_history() {
    let directory = tempdir().expect("temporary directory");
    let legacy = directory.path().join("xunqi.db");
    std::fs::write(&legacy, b"legacy-v1-placeholder").expect("legacy fixture");
    let before = std::fs::read(&legacy).expect("legacy before");

    let app = Application::open(directory.path().join("v2").join("xunqi.db"))
        .expect("open v2 application");
    assert!(app.list_tasks().expect("empty v2 queue").is_empty());
    assert_eq!(std::fs::read(&legacy).expect("legacy after"), before);
}

#[test]
fn interrupted_output_returns_to_ready_instead_of_refetching_content() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("xunqi.db");
    let app = Application::open(&database).expect("open application");
    let task = app
        .submit_links("https://mp.weixin.qq.com/s/interrupted-export")
        .expect("submit link")
        .tasks
        .remove(0);
    drop(app);

    let connection = Connection::open(&database).expect("open sqlite database");
    connection
        .execute(
            "UPDATE active_tasks SET status = 'exporting' WHERE id = ?1",
            [task.id],
        )
        .expect("mark interrupted export");
    drop(connection);

    let reopened = Application::open(&database).expect("reopen application");
    let recovered = reopened
        .get_task_detail(task.id)
        .expect("read recovered task");
    assert_eq!(recovered.task.status, CaptureStatus::Ready);
    assert!(recovered.task.status_detail.contains("重新导出"));
}
