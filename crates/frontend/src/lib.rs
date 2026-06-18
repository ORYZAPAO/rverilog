pub mod error;
pub mod hir;
pub mod lower;

pub use error::FrontendError;
pub use lower::parse_files;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_counter4() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/counter4");
        let files = vec![
            root.join("counter4.v"),
            root.join("tb.v"),
        ];
        let design = parse_files(&files, &[], &[]).expect("parse failed");
        assert_eq!(design.modules.len(), 2, "expected 2 modules");

        let counter = design.modules.get("counter4").expect("counter4 missing");
        assert_eq!(counter.ports.len(), 3, "expected 3 ports: clk, rst, count");
        assert_eq!(counter.alwayses.len(), 1, "expected 1 always block");

        let names: Vec<_> = counter.ports.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"clk"));
        assert!(names.contains(&"rst"));
        assert!(names.contains(&"count"));

        let tb = design.modules.get("tb_counter4").expect("tb_counter4 missing");
        assert!(!tb.instances.is_empty(), "expected instance in tb");
    }
}
