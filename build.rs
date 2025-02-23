fn main() {
    // Replace this path with the actual directory where libraw.lib is located.
    println!("cargo:rustc-link-search=native=C:\\Users\\hanba\\LibRaw-0.21.3\\lib");
    // Instruct the linker to link against libraw (the name should match the library name, e.g. "libraw" if the file is libraw.lib).
    println!("cargo:rustc-link-lib=dylib=libraw");
}
