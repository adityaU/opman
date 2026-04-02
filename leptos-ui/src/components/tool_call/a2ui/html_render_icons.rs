//! SVG icon helpers shared across A2UI HTML renderers.

pub(super) fn svg_icon(name: &str, size: u32) -> String {
    let body = match name {
        "check-circle" => "<path d=\"M22 11.08V12a10 10 0 1 1-5.93-9.14\"/><polyline points=\"22 4 12 14.01 9 11.01\"/>",
        "alert-triangle" => "<path d=\"M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z\"/><line x1=\"12\" y1=\"9\" x2=\"12\" y2=\"13\"/><line x1=\"12\" y1=\"17\" x2=\"12.01\" y2=\"17\"/>",
        "x-circle" => "<circle cx=\"12\" cy=\"12\" r=\"10\"/><line x1=\"15\" y1=\"9\" x2=\"9\" y2=\"15\"/><line x1=\"9\" y1=\"9\" x2=\"15\" y2=\"15\"/>",
        "info" => "<circle cx=\"12\" cy=\"12\" r=\"10\"/><line x1=\"12\" y1=\"16\" x2=\"12\" y2=\"12\"/><line x1=\"12\" y1=\"8\" x2=\"12.01\" y2=\"8\"/>",
        "external-link" => "<path d=\"M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6\"/><polyline points=\"15 3 21 3 21 9\"/><line x1=\"10\" y1=\"14\" x2=\"21\" y2=\"3\"/>",
        _ => "",
    };
    format!(
        "<svg width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\" fill=\"none\" \
         stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" \
         stroke-linejoin=\"round\">{}</svg>",
        size, size, body
    )
}

pub(super) fn level_icon_html(level: &str, size: u32) -> String {
    match level {
        "success" => svg_icon("check-circle", size),
        "warning" => svg_icon("alert-triangle", size),
        "error" => svg_icon("x-circle", size),
        _ => svg_icon("info", size),
    }
}
