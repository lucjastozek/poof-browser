use std::{env, fs::File, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=poof-logo.svg");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let tree = resvg::usvg::Tree::from_data(
        include_bytes!("poof-logo.svg"),
        &resvg::usvg::Options::default(),
    ).expect("poof-logo.svg must be a valid SVG");
    let size = tree.size();
    let width = size.width();
    let height = size.height();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256)
        .expect("could not allocate app icon");
    let scale = (256.0 / width).min(256.0 / height);
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale)
        .post_translate((256.0 - width * scale) / 2.0, (256.0 - height * scale) / 2.0);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let image = ico::IconImage::from_rgba_data(256, 256, pixmap.data().to_vec());
    let entry = ico::IconDirEntry::encode(&image).expect("could not encode app icon");
    let mut directory = ico::IconDir::new(ico::ResourceType::Icon);
    directory.add_entry(entry);

    let icon_path = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("poof.ico");
    let mut file = File::create(&icon_path).expect("could not create app icon file");
    directory.write(&mut file).expect("could not write app icon file");

    winres::WindowsResource::new()
        .set_icon(icon_path.to_str().unwrap())
        .compile()
        .expect("could not embed Poof application icon");
}
