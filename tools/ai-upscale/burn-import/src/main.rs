//! Offline feasibility tool; this does not enable a product backend.
fn main() {
    let mut args = std::env::args().skip(1);
    let model = args.next().expect("expected ONNX model path");
    let output = args.next().expect("expected generated output directory");
    assert!(args.next().is_none(), "unexpected argument");
    burn_onnx::ModelGen::new()
        .input(&model)
        .out_dir(&output)
        .run_from_cli();
}
