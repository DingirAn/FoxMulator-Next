fn main() {
    slint_build::compile("src/Ui.slint").expect("failed to compile Ui.slint");
    println!("cargo:rerun-if-changed=src/Ui.slint");
}
