use ply_engine::prelude::*;

fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: miniquad::conf::Conf {
            window_title: "Wuddle".to_owned(),
            window_width: 1100,
            window_height: 850,
            high_dpi: true,
            sample_count: 4,
            platform: miniquad::conf::Platform {
                webgl_version: miniquad::conf::WebGLVersion::WebGL2,
                ..Default::default()
            },
            ..Default::default()
        },
        draw_call_vertex_capacity: 100000,
        draw_call_index_capacity: 100000,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    static DEFAULT_FONT: FontAsset = FontAsset::Path("assets/fonts/Inter-Regular.ttf");
    let mut ply = Ply::<()>::new(&DEFAULT_FONT).await;

    loop {
        clear_background(BLACK);

        let mut ui = ply.begin();

        // Root container
        ui.element()
            .width(grow!())
            .height(grow!())
            .background_color(0x1a1a2e)
            .children(|ui| {
                // Title bar
                ui.element()
                    .width(grow!())
                    .height(fixed!(48.0))
                    .background_color(0x16213e)
                    .layout(|l| l.padding((0, 16, 0, 16)).align(Left, CenterY))
                    .children(|ui| {
                        ui.text("Wuddle v3.0.0-alpha", |t| {
                            t.font_size(20).color(0xe0e0e0)
                        });
                    });

                // Body placeholder
                ui.element()
                    .width(grow!())
                    .height(grow!())
                    .layout(|l| l.padding(16).align(CenterX, CenterY))
                    .children(|ui| {
                        ui.text("Ply frontend scaffold — work in progress", |t| {
                            t.font_size(14).color(0x888888)
                        });
                    });
            });

        ui.show(|_| {}).await;
        next_frame().await;
    }
}
