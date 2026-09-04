// Embeds the Windows executable icon and version metadata. The icon is the
// lacodda line mark exported to a multi-size .ico, one level per size (S for
// 16/24, M for 32/48, L for 64 and up), so Explorer picks the right drawing
// for each view.
//
// The icon arrives with the mark, which is chosen after the repository is
// founded; until then the build must not fail on its absence.
fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        if std::path::Path::new("assets/icon.ico").exists() {
            winresource::WindowsResource::new()
                .set_icon("assets/icon.ico")
                .compile()
                .expect("failed to embed the Windows resources");
        }
    }
}
