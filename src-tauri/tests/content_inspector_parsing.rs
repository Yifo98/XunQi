use xunqi_lib::{ContentInspector, DetectedVideoKind, PublicContentInspector};

#[test]
fn public_wechat_article_markup_is_normalized_into_a_readable_snapshot() {
    let html = r#"
      <html>
        <head>
          <meta property="og:title" content="公开文章标题">
          <meta property="og:url" content="https://mp.weixin.qq.com/s/canonical">
        </head>
        <body>
          <div id="js_name">示例科技周报</div>
          <div id="publish_time">2026年7月13日 12:30</div>
          <div id="js_content">
            <p>这是第一段公开正文，包含足够的信息用于离线阅读。</p>
            <img data-src="https://mmbiz.qpic.cn/demo-first.png?wx_fmt=png&amp;from=appmsg" src="https://mmbiz.qpic.cn/remote-fallback.jpg">
            <img src="http://127.0.0.1/private.png" srcset="https://127.0.0.1/private-2x.png 2x">
            <blockquote>这是一段原文引用。</blockquote>
            <ul><li>原文列表项目</li></ul>
            <p>这是第二段公开正文。</p>
            <script>window.shouldNeverBeExported = true;</script>
          </div>
        </body>
      </html>
    "#;

    let snapshot =
        PublicContentInspector::inspect_article_html("https://mp.weixin.qq.com/s/shared", html)
            .expect("inspect article fixture");
    assert_eq!(snapshot.title, "公开文章标题");
    assert_eq!(snapshot.author, "示例科技周报");
    assert_eq!(
        snapshot.published_at.as_deref(),
        Some("2026年7月13日 12:30")
    );
    assert_eq!(
        snapshot.canonical_url,
        "https://mp.weixin.qq.com/s/canonical"
    );
    assert_eq!(
        snapshot.body_markdown,
        "这是第一段公开正文，包含足够的信息用于离线阅读。\n\n> 这是一段原文引用。\n\n- 原文列表项目\n\n这是第二段公开正文。"
    );
    assert!(snapshot
        .body_html
        .contains("<blockquote>这是一段原文引用。</blockquote>"));
    assert!(snapshot
        .body_html
        .contains("<ul><li>原文列表项目</li></ul>"));
    assert!(snapshot.body_html.contains("data-xunqi-image=\"0\""));
    assert!(!snapshot.body_html.contains("remote-fallback"));
    assert!(!snapshot.body_html.contains("127.0.0.1"));
    assert!(!snapshot.body_html.contains("srcset"));
    assert!(!snapshot.body_html.contains("<script"));
    assert!(!snapshot.body_html.contains("shouldNeverBeExported"));
    assert_eq!(
        snapshot.image_urls,
        vec!["https://mmbiz.qpic.cn/demo-first.png?wx_fmt=png&from=appmsg"]
    );
}

#[test]
fn wechat_text_post_script_payload_is_normalized_into_a_readable_snapshot() {
    let html = r#"
      <html>
        <head><title>微信公众号文字内容</title></head>
        <body>
          <script>
            window.appmsg_type = '10002';
            window.textPost = {
              nick_name: '示例科技周报',
              title: '测试文字内容：本地整理方法。',
              content_noencode: '第一段包含\x3ca href=\x22https://mp.weixin.qq.com/mp/readtemplate?t=pages/link_mid_jump\x22\x3e@示例作者\x3c/a\x3e的公开测试文字。\x0a\x0a第二段是完整公开正文，应该可以直接阅读和导出。',
              create_time: '2026-07-14 20:15',
              link: 'https://mp.weixin.qq.com/s/public-text-demo?nwr_flag=1',
              type: '10002' * 1
            };
          </script>
        </body>
      </html>
    "#;

    let snapshot = PublicContentInspector::inspect_article_html(
        "https://mp.weixin.qq.com/s/public-text-demo",
        html,
    )
    .expect("inspect text-post fixture");

    assert_eq!(snapshot.title, "测试文字内容：本地整理方法。");
    assert_eq!(snapshot.author, "示例科技周报");
    assert_eq!(snapshot.published_at.as_deref(), Some("2026-07-14 20:15"));
    assert_eq!(
        snapshot.canonical_url,
        "https://mp.weixin.qq.com/s/public-text-demo?nwr_flag=1"
    );
    assert!(snapshot
        .body_markdown
        .contains("第一段包含@示例作者的公开测试文字"));
    assert!(snapshot.body_markdown.contains("第二段是完整公开正文"));
    assert!(snapshot.body_html.contains("<p>"));
    assert!(snapshot.body_html.contains("@示例作者"));
    assert!(snapshot.image_urls.is_empty());
}

#[test]
#[ignore = "requires XUNQI_REAL_TEXT_POST_URL for a user-authorized live page"]
fn live_wechat_text_post_can_be_read_without_cookie_or_login_state() {
    let url = std::env::var("XUNQI_REAL_TEXT_POST_URL")
        .expect("set XUNQI_REAL_TEXT_POST_URL to an authorized public text post");
    let snapshot = PublicContentInspector
        .inspect_article(&url)
        .expect("inspect live WeChat text post");

    assert!(!snapshot.title.trim().is_empty());
    assert!(!snapshot.author.trim().is_empty());
    assert!(snapshot.word_count > 100);
    assert!(!snapshot.body_markdown.trim().is_empty());
}

#[test]
fn public_html5_video_candidates_are_separated_from_manifests_and_blob_media() {
    let html = r#"
      <html>
        <head>
          <title>公开视频演示</title>
          <meta property="og:video" content="https://cdn.example.com/master.m3u8">
        </head>
        <body>
          <video src="/media/clip.mp4?token=public">
            <source src="blob:https://example.com/runtime-stream">
          </video>
        </body>
      </html>
    "#;

    let inspection =
        PublicContentInspector::inspect_video_html("https://example.com/watch/one", html)
            .expect("inspect video fixture");
    assert_eq!(inspection.page_title, "公开视频演示");
    let direct = inspection
        .candidates
        .iter()
        .find(|candidate| candidate.kind == DetectedVideoKind::DirectFile)
        .expect("direct candidate");
    assert_eq!(
        direct.url,
        "https://example.com/media/clip.mp4?token=public"
    );
    assert!(direct.downloadable);
    assert!(inspection.candidates.iter().any(|candidate| {
        candidate.kind == DetectedVideoKind::HlsPlaylist && !candidate.downloadable
    }));
    assert!(inspection.candidates.iter().any(|candidate| {
        candidate.kind == DetectedVideoKind::Unsupported
            && candidate.url.starts_with("blob:")
            && !candidate.downloadable
    }));
}

#[test]
fn wechat_short_video_metadata_reports_the_public_playback_limit() {
    let short_response = r#"
      {
        "data": {
          "authorInfo": { "nickname": "影像测试频道" },
          "feedInfo": {
            "description": "测试视频：公开接口限制演示",
            "createtime": 1781778600,
            "coverUrl": "https://finder.video.qq.com/251/cover/stodownload?token=image"
          },
          "sceneInfo": { "dynamicExportId": "export/public-id" }
        },
        "errCode": 0,
        "errMsg": ""
      }
    "#;
    let feed_response = r#"
      {
        "data": {
          "errMsg": { "type": 2, "title": "此内容暂时无法播放" }
        },
        "errCode": 0,
        "errMsg": ""
      }
    "#;

    let inspection = PublicContentInspector::inspect_wechat_finder_json(
        "https://channels.weixin.qq.com/finder-preview/pages/sph?id=public-demo",
        short_response,
        Some(feed_response),
    )
    .expect("inspect public finder responses");

    assert_eq!(inspection.source_name, "影像测试频道");
    assert!(inspection.page_title.contains("公开接口限制演示"));
    assert_eq!(inspection.description, inspection.page_title);
    assert!(inspection.published_at.is_some());
    assert_eq!(
        inspection.cover_image_url.as_deref(),
        Some("https://finder.video.qq.com/251/cover/stodownload?token=image")
    );
    assert!(inspection.candidates.is_empty());
    assert!(inspection.limitation.contains("此内容暂时无法播放"));
    assert!(inspection.limitation.contains("没有提供视频地址"));
}

#[test]
fn wechat_public_api_video_fields_allow_extensionless_direct_downloads() {
    let response = r#"
      {
        "data": {
          "authorInfo": { "nickname": "公开视频作者" },
          "feedInfo": {
            "description": "公开视频标题",
            "coverUrl": "https://finder.video.qq.com/251/cover/stodownload?token=image",
            "h264VideoInfo": {
              "videoUrl": "https://finder.video.qq.com/251/video/stodownload?token=video"
            }
          }
        },
        "errCode": 0,
        "errMsg": ""
      }
    "#;

    let inspection = PublicContentInspector::inspect_wechat_finder_json(
        "https://channels.weixin.qq.com/finder-preview/pages/sph?id=public",
        response,
        None,
    )
    .expect("inspect public video field");

    assert_eq!(inspection.candidates.len(), 1);
    let candidate = &inspection.candidates[0];
    assert_eq!(candidate.kind, DetectedVideoKind::DirectFile);
    assert!(candidate.downloadable);
    assert!(candidate.url.contains("token=video"));
    assert!(!candidate.url.contains("token=image"));
}
