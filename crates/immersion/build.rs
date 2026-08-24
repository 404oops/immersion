fn main() {
    println!("cargo:rerun-if-changed=assets/icons/Immersion.ico");
    // Windows exe resources (icon + version block), like the old
    // packaging/windows/Immersion.rc.in.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icons/Immersion.ico");
        resource.set("ProductName", "Immersion");
        resource.set("FileDescription", "Immersion - Music Project Versioning");
        resource.set("LegalCopyright", "Copyright 404oops");
        resource
            .compile()
            .expect("failed to compile Windows resources");
    }
}
