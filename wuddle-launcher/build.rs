fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest_dir =
            std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let output_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let package_version = std::env::var("CARGO_PKG_VERSION").unwrap();
        let icon_path = manifest_dir.join("assets").join("wuddle.ico");
        let mut numeric_version = package_version
            .split(|character: char| !character.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse::<u16>().ok())
            .take(4)
            .collect::<Vec<_>>();
        numeric_version.resize(4, 0);
        let escaped_icon_path = icon_path.to_string_lossy().replace('\\', "\\\\");
        let resource_path = output_dir.join("wuddle-launcher-resources.rc");
        let resource = format!(
            r#"
1 ICON "{escaped_icon_path}"

1 VERSIONINFO
FILEVERSION {major},{minor},{patch},{build}
PRODUCTVERSION {major},{minor},{patch},{build}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName", "ZythDr\0"
            VALUE "FileDescription", "Wuddle Launcher\0"
            VALUE "FileVersion", "{package_version}\0"
            VALUE "InternalName", "Wuddle Launcher\0"
            VALUE "OriginalFilename", "Wuddle.exe\0"
            VALUE "ProductName", "Wuddle\0"
            VALUE "ProductVersion", "{package_version}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#,
            major = numeric_version[0],
            minor = numeric_version[1],
            patch = numeric_version[2],
            build = numeric_version[3],
        );

        std::fs::write(&resource_path, resource)
            .expect("write generated Windows launcher resources");

        embed_resource::compile(&resource_path, embed_resource::NONE)
            .manifest_required()
            .expect("compile Windows launcher icon and version metadata");
    }
}
