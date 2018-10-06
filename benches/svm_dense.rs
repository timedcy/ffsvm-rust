#![feature(test, try_from)]

// This is a little weird, see
// https://github.com/rust-lang/rust/pull/54116#issuecomment-422294107
// for details
extern crate test;

mod svm_dense {
    use crate::test::Bencher;
    use ffsvm::{DenseSVM, ModelFile, Predict, Problem};
    use std::convert::TryFrom;

    /// Produces a test case run for benchmarking
    #[allow(dead_code)]
    fn produce_testcase(
        svm_type: &str,
        kernel_type: &str,
        total_sv: u32,
        num_attributes: u32,
    ) -> impl FnMut() {
        let raw_model = ModelFile::random_dense(svm_type, kernel_type, total_sv, num_attributes);
        let svm = DenseSVM::try_from(&raw_model).unwrap();
        let mut problem = Problem::from(&svm);
        let problem_mut = problem.features().as_slice_mut();

        for i in 0..num_attributes {
            problem_mut[i as usize] = i as f32;
        }

        move || {
            (&svm)
                .predict_value(&mut problem)
                .expect("This should work")
        }
    }

    // RBF

    #[bench]
    fn predict_rbf_sv128_attr16(b: &mut Bencher) {
        b.iter(produce_testcase("c_svc", "rbf", 128, 16));
    }

    #[bench]
    fn predict_rbf_sv1024_attr16(b: &mut Bencher) {
        b.iter(produce_testcase("c_svc", "rbf", 1024, 16));
    }

    #[bench]
    fn predict_rbf_sv1024_attr1024(b: &mut Bencher) {
        b.iter(produce_testcase("c_svc", "rbf", 1024, 1024));
    }

    // Linear

    #[bench]
    fn predict_linear_sv128_attr16(b: &mut Bencher) {
        b.iter(produce_testcase("c_svc", "linear", 128, 16));
    }

    #[bench]
    fn predict_linear_sv1024_attr16(b: &mut Bencher) {
        b.iter(produce_testcase("c_svc", "linear", 1024, 16));
    }

    #[bench]
    fn predict_linear_sv1024_attr1024(b: &mut Bencher) {
        b.iter(produce_testcase("c_svc", "linear", 1024, 1024));
    }

    // Poly

    #[bench]
    fn predict_poly_sv128_attr16(b: &mut Bencher) {
        b.iter(produce_testcase("c_svc", "polynomial", 128, 16));
    }

    #[bench]
    fn predict_poly_sv1024_attr16(b: &mut Bencher) {
        b.iter(produce_testcase("c_svc", "polynomial", 1024, 16));
    }

    #[bench]
    fn predict_poly_sv1024_attr1024(b: &mut Bencher) {
        b.iter(produce_testcase("c_svc", "polynomial", 1024, 1024));
    }

    // Sigmoid

    #[bench]
    fn predict_sigmoid_sv128_attr16(b: &mut Bencher) {
        b.iter(produce_testcase("c_svc", "sigmoid", 128, 16));
    }

    #[bench]
    fn predict_sigmoid_sv1024_attr16(b: &mut Bencher) {
        b.iter(produce_testcase("c_svc", "sigmoid", 1024, 16));
    }

    #[bench]
    fn predict_sigmoid_sv1024_attr1024(b: &mut Bencher) {
        b.iter(produce_testcase("c_svc", "sigmoid", 1024, 1024));
    }

}
