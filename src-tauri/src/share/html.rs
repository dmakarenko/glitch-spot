// ABOUTME: Server-side HTML rendering for the /share/{id}/{token}/ landing page.
// ABOUTME: Pure function — no I/O, no templating engine, just format!() over Snapshot fields.

use crate::snapshot::Snapshot;

pub fn render_share_page(snap: &Snapshot) -> String {
    let mut notes_list = String::new();
    for a in &snap.annotations {
        notes_list.push_str(&format!("    <li>{}</li>\n", escape_html(&a.note)));
    }
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Glitch Spot — {id}</title>
  <style>
    body {{ font: 14px/1.45 -apple-system, system-ui, sans-serif; max-width: 960px; margin: 32px auto; padding: 0 16px; color: #222; }}
    h1 {{ font-size: 16px; font-weight: 600; margin: 0 0 16px; color: #555; }}
    img {{ max-width: 100%; height: auto; border: 1px solid #ddd; border-radius: 4px; }}
    ol {{ margin-top: 24px; padding-left: 28px; }}
    ol li {{ margin: 8px 0; }}
    footer {{ margin-top: 32px; font-size: 12px; color: #888; }}
    footer a {{ color: #555; }}
  </style>
</head>
<body>
  <h1>Glitch Spot snapshot — {id}</h1>
  <img src="annotated.png" alt="annotated screenshot">
  <ol>
{notes}  </ol>
  <footer>
    <a href="snapshot.json">Open raw JSON</a>
  </footer>
</body>
</html>
"#,
        id = escape_html(&snap.id),
        notes = notes_list,
    )
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Annotation, Context, Coords, Images, OsInfo, ScreenInfo, Snapshot};

    fn fake_snapshot() -> Snapshot {
        Snapshot {
            id: "snap_x01".to_string(),
            created_at: "2026-05-26T12:00:00Z".to_string(),
            mode: "frozen".to_string(),
            annotations: vec![
                Annotation { id: "a1".into(), shape: "rect".into(), coords: Coords { x: 0.0, y: 0.0, w: 10.0, h: 10.0 }, note: "the close button is misaligned".into(), label: "01".into() },
                Annotation { id: "a2".into(), shape: "rect".into(), coords: Coords { x: 20.0, y: 0.0, w: 10.0, h: 10.0 }, note: "this label has a typo".into(), label: "02".into() },
            ],
            context: Context {
                screen: ScreenInfo { width: 1440, height: 900, scale_factor: 2.0, display_index: 0 },
                os: OsInfo { platform: "macos".into(), version: "26.0".into() },
            },
            images: Images { clean: "/tmp/c.png".into(), annotated: Some("/tmp/a.png".into()) },
        }
    }

    #[test]
    fn includes_annotated_img_tag() {
        let html = render_share_page(&fake_snapshot());
        assert!(html.contains(r#"<img src="annotated.png""#), "html missing annotated img tag: {html}");
    }

    #[test]
    fn lists_notes_in_order() {
        let html = render_share_page(&fake_snapshot());
        let pos_first = html.find("the close button is misaligned").expect("first note missing");
        let pos_second = html.find("this label has a typo").expect("second note missing");
        assert!(pos_first < pos_second, "notes rendered out of order");
    }

    #[test]
    fn includes_raw_json_link() {
        let html = render_share_page(&fake_snapshot());
        assert!(html.contains(r#"<a href="snapshot.json""#), "html missing raw JSON link");
    }

    #[test]
    fn escapes_html_entities_in_notes() {
        let mut snap = fake_snapshot();
        snap.annotations[0].note = "<script>alert(1)</script>".into();
        let html = render_share_page(&snap);
        assert!(!html.contains("<script>"), "raw <script> made it into rendered HTML");
        assert!(html.contains("&lt;script&gt;"), "expected escaped &lt;script&gt;: {html}");
    }

    #[test]
    fn includes_snapshot_id() {
        let html = render_share_page(&fake_snapshot());
        assert!(html.contains("snap_x01"), "snapshot id not in page title/heading");
    }
}
