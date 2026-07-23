fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest_dir =
            std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let output_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let icon_path = manifest_dir.join("assets").join("wuddle.ico");
        let escaped_icon_path = icon_path.to_string_lossy().replace('\\', "\\\\");
        let resource_path = output_dir.join("wuddle-launcher-resources.rc");

        std::fs::write(
            &resource_path,
            format!("IDI_ICON1 ICON \"{escaped_icon_path}\"\n"),
        )
        .expect("write generated Windows launcher resources");

        embed_resource::compile(&resource_path, embed_resource::NONE)
            .manifest_required()
            .expect("compile Windows launcher icon");
    }
}
