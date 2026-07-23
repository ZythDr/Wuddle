use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=assets/icons/wuddle.ico");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let output_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let package_version = env::var("CARGO_PKG_VERSION").unwrap();
    let icon_path = manifest_dir.join("assets").join("icons").join("wuddle.ico");

    let mut numeric_version = package_version
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u16>().ok())
        .take(4)
        .collect::<Vec<_>>();
    numeric_version.resize(4, 0);

    let escaped_icon_path = icon_path.to_string_lossy().replace('\\', "\\\\");
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
            VALUE "FileDescription", "Wuddle\0"
            VALUE "FileVersion", "{package_version}\0"
            VALUE "InternalName", "Wuddle\0"
            VALUE "OriginalFilename", "Wuddle-bin.exe\0"
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

    let resource_path = output_dir.join("wuddle-resources.rc");
    fs::write(&resource_path, resource).expect("write generated Windows resources");
    embed_resource::compile(&resource_path, embed_resource::NONE)
        .manifest_required()
        .expect("compile Windows icon and version metadata");
}
