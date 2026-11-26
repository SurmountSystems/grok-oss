//! An image attached while planning is painted in the planning view.
//!
//! The painted string is the same `[Image #N]` chip the L1 transcript uses
//! (`prompt_images::display_text`). OCR text does not replace that picture.
//! A missing-image notice does not replace it. The bytes stay on the prompt.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::actions::ActionRegistry;
use crate::scrollback::render::ScratchBuffer;
use crate::views::plan_approval_view::{ExitPlanModeExtRequest, PlanApprovalViewState};
use crate::views::prompt_widget::StashedPrompt;

use super::{AgentView, AppRenderParams, BannerSlotParams, test_fixtures};

fn draw(agent: &mut AgentView) -> String {
    let registry = ActionRegistry::defaults();
    let area = Rect::new(0, 0, 100, 40);
    let mut buf = Buffer::empty(area);
    let mut scratch = ScratchBuffer::new();
    agent.draw(
        area,
        &mut buf,
        &registry,
        &mut scratch,
        None,
        false,
        BannerSlotParams::none(),
        false,
        false,
        &mut Vec::new(),
        AppRenderParams::default(),
    );
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .filter_map(|x| buf.cell((x, y)).map(|cell| cell.symbol().to_string()))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn png_8x8() -> Vec<u8> {
    let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        image::ImageBuffer::from_pixel(8, 8, image::Rgba([1, 2, 3, 255]));
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("8x8 png");
    buf
}

/// Falsifying check: planning mode paints no image for an attached picture.
#[test]
fn planning_mode_paints_an_attached_image() {
    let _theme = crate::theme::cache::pin_theme();
    let dir = tempfile::tempdir().expect("tempdir");
    let image_path = dir.path().join("screen.png");
    std::fs::write(&image_path, png_8x8()).expect("write plan image");

    let mut agent = test_fixtures::make_agent();
    let (tx, _rx) = tokio::sync::oneshot::channel();
    let request = ExitPlanModeExtRequest {
        session_id: "test-session".into(),
        tool_call_id: "call-plan-image".into(),
        plan_content: Some("# Plan\n\nKeep the picture\n".into()),
    };
    agent.plan_approval_view = Some(PlanApprovalViewState::new(
        request,
        StashedPrompt::default(),
        tx,
    ));
    agent.plan_mode_active = true;
    agent.show_plan_preview_if_available();
    assert!(
        agent.line_viewer.is_some(),
        "the planning view is open before the image is attached"
    );
    assert!(
        agent.try_attach_tui_screenshot_for_plan(&image_path),
        "planning mode accepts the attached image"
    );
    assert!(
        agent.prompt.images.iter().any(|image| image
            .encoded_bytes
            .as_ref()
            .is_some_and(|bytes| !bytes.is_empty())),
        "plan mode must not drop the image bytes"
    );

    let screen = draw(&mut agent);
    let painted = agent
        .line_viewer
        .as_ref()
        .and_then(|viewer| viewer.plan_ref())
        .map(|plan| plan.revision_prompt_text.clone())
        .unwrap_or_default();
    assert!(
        painted.contains("[Image #1]"),
        "planning mode paints no image for an attached picture; got {painted:?}"
    );
    assert!(
        screen.contains("[Image #1]"),
        "planning mode paints no image for an attached picture; screen omitted the chip:\n{screen}"
    );
    assert!(
        !painted.contains("not attached"),
        "a missing-image notice must not replace the picture; got {painted:?}"
    );
    assert!(
        !painted.contains("tesseract"),
        "OCR text must not replace the picture; got {painted:?}"
    );
}
