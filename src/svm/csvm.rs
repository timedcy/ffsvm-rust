use std::convert::TryFrom;

use crate::errors::SVMError;
use crate::parser::ModelFile;
use crate::random::*;
use crate::svm::class::Class;
use crate::svm::kernel::{Kernel, Linear, Rbf};
use crate::svm::problem::Problem;
use crate::svm::Probabilities;
use crate::util::set_all;
use crate::vectors::Triangular;

/// Generic support vector machine, template for [RbfCSVM].
///
/// The SVM holds a kernel, class information and all other numerical data read from
/// the [ModelFile]. It implements [Predict] to predict [Problem] instances.
///
/// # Creating a SVM
///
/// The only SVM currently implemented is the [RbfCSVM]. It can be constructed from a
/// [ModelFile] like this:
///
/// ```ignore
/// let svm = RbfCSVM::try_from(&model)!;
/// ```
///
pub struct CSVM {
    /// Total number of support vectors
    crate num_total_sv: usize,

    /// Number of attributes per support vector
    crate num_attributes: usize,

    crate rho: Triangular<f64>,

    crate probabilities: Option<Probabilities>,

    /// SVM specific data needed for classification
    crate kernel: Box<dyn Kernel>,

    /// All classes
    crate classes: Vec<Class>,
}

impl CSVM {
    /// Computes the kernel values for this problem
    crate fn compute_kernel_values(&self, problem: &mut Problem) {
        // Get current problem and decision values array
        let features = &problem.features;
        let kernel_values = &mut problem.kernel_values;

        // Compute kernel values per class
        for (i, class) in self.classes.iter().enumerate() {
            let kvalues = kernel_values.row_as_flat_mut(i);

            self.kernel
                .compute(&class.support_vectors, features, kvalues);
        }
    }

    // This is pretty much copy-paste of `multiclass_probability` from libSVM which we need
    // to be compatibly for predicting probability for multiclass SVMs. The method is in turn
    // based on Method 2 from the paper "Probability Estimates for Multi-class
    // Classification by Pairwise Coupling", Journal of Machine Learning Research 5 (2004) 975-1005,
    // by Ting-Fan Wu, Chih-Jen Lin and Ruby C. Weng.
    crate fn compute_multiclass_probabilities(
        &self,
        problem: &mut Problem,
    ) -> Result<(), SVMError> {
        let num_classes = self.classes.len();
        let max_iter = 100.max(num_classes);
        let mut q = problem.q.flat_mut();
        let qp = &mut problem.qp;
        let eps = 0.005 / num_classes as f64; // Magic number .005 comes from libSVM.
        let pairwise = problem.pairwise.flat();

        // We first build up matrix Q as defined in (14) in the paper above. Q should have
        // the property of being a transition matrix for a Markov Chain.
        for t in 0..num_classes {
            problem.probabilities[t] = 1.0 / num_classes as f64;

            q[(t, t)] = 0.0;

            for j in 0..t {
                q[(t, t)] += pairwise[(j, t)] * pairwise[(j, t)];
                q[(t, j)] = q[(j, t)];
            }

            for j in t + 1..num_classes {
                q[(t, t)] += pairwise[(j, t)] * pairwise[(j, t)];
                q[(t, j)] = -pairwise[(j, t)] * pairwise[(t, j)];
            }
        }

        // We now try to satisfy (21), (23) and (24) in the paper above.
        for i in 0..=max_iter {
            let mut pqp = 0.0;

            for t in 0..num_classes {
                qp[t] = 0.0;

                for j in 0..num_classes {
                    qp[t] += q[(t, j)] * problem.probabilities[j];
                }

                pqp += problem.probabilities[t] * qp[t];
            }

            // Check if we fulfilled our abort criteria, which seems to be related
            // to (21).
            let mut max_error = 0.0;

            for item in qp.iter() {
                let error = (*item - pqp).abs();

                if error > max_error {
                    max_error = error;
                }
            }

            if max_error < eps {
                break;
            }

            // In case we are on the last iteration round past the threshold
            // we know something went wrong. Signal we exceeded the threshold.
            if i == max_iter {
                return Err(SVMError::IterationsExceeded);
            }

            // This seems to be the main function performing (23) and (24).
            for t in 0..num_classes {
                let diff = (-qp[t] + pqp) / q[(t, t)];

                problem.probabilities[t] += diff;
                pqp = (pqp + diff * (diff * q[(t, t)] + 2.0 * qp[t])) / (1.0 + diff) / (1.0 + diff);

                for j in 0..num_classes {
                    qp[j] = (qp[j] + diff * q[(t, j)]) / (1.0 + diff);
                    problem.probabilities[j] /= 1.0 + diff;
                }
            }
        }

        Ok(())
    }

    /// Based on kernel values, computes the decision values for this problem.
    crate fn compute_decision_values(&self, problem: &mut Problem) {
        // Reset all votes
        set_all(&mut problem.vote, 0);

        // Since classification is symmetric, if we have N classes, we only need to go through
        // (N * N - 1) - 1 cases. For example for 4 classes we do:
        //
        //          j --->
        //          0    1   2   3
        //    i 0        x   x   x
        //    | 1            x   x
        //    v 2                x
        //      3
        //
        // For each valid combination (e.g., i:1, j:2), we then need to compute
        // the decision values, which consists of two parts:
        //
        // a) The coefficients of class(1) related to class(2) and
        // b) The coefficients of class(2) related to class(1).
        //
        // Both a) and b) are multiplied with the computed kernel values and summed,
        // and eventually used to compute on which side we are.

        for i in 0..self.classes.len() {
            for j in (i + 1)..self.classes.len() {
                let sv_coef0 = self.classes[i].coefficients.row(j - 1);
                let sv_coef1 = self.classes[j].coefficients.row(i);

                let kvalues0 = problem.kernel_values.row(i);
                let kvalues1 = problem.kernel_values.row(j);

                let sum0 = sv_coef0
                    .iter()
                    .zip(kvalues0)
                    .map(|(a, b)| (*a * *b).sum())
                    .sum::<f64>();

                let sum1 = sv_coef1
                    .iter()
                    .zip(kvalues1)
                    .map(|(a, b)| (*a * *b).sum())
                    .sum::<f64>();

                let sum = sum0 + sum1 - self.rho[(i, j)];
                let index_to_vote = if sum > 0.0 { i } else { j };

                problem.decision_values[(i, j)] = sum;
                problem.vote[index_to_vote] += 1;
            }
        }
    }

    /// Finds the class index for a given label.
    ///
    /// # Description
    ///
    /// This method takes a `label` as defined in the libSVM training model
    /// and returns the internal `index` where this label resides. The index
    /// equals the [Problem]'s `.probabilities` index where that label's
    /// probability can be found.
    ///
    /// # Returns
    ///
    /// If the label was found its index returned in the [Option]. Otherwise `None`
    /// is returned.
    ///
    pub fn class_index_for_label(&self, label: u32) -> Option<usize> {
        for (i, class) in self.classes.iter().enumerate() {
            if class.label != label {
                continue;
            }

            return Some(i);
        }

        None
    }

    /// Returns the class label for a given index.
    ///
    /// # Description
    ///
    /// The inverse of [SVM::class_index_for_label], this function returns the class label
    /// associated with a certain internal index. The index equals the [Problem]'s
    /// `.probabilities` index where a label's probability can be found.
    ///
    /// # Returns
    ///
    /// If the index was found it is returned in the [Option]. Otherwise `None`
    /// is returned.
    pub fn class_label_for_index(&self, index: usize) -> Option<u32> {
        if index >= self.classes.len() {
            None
        } else {
            Some(self.classes[index].label)
        }
    }

    /// Returns number of attributes, reflecting the libSVM model.
    pub fn attributes(&self) -> usize {
        self.num_attributes
    }

    /// Returns number of classes, reflecting the libSVM model.
    pub fn classes(&self) -> usize {
        self.classes.len()
    }
}

impl RandomSVM for CSVM {
    fn random<K>(num_classes: usize, num_sv_per_class: usize, num_attributes: usize) -> Self
    where
        K: Kernel + Random + 'static,
    {
        let num_total_sv = num_classes * num_sv_per_class;
        let classes = (0..num_classes)
            .map(|class| {
                Class::with_parameters(num_classes, num_sv_per_class, num_attributes, class as u32)
                    .randomize()
            }).collect::<Vec<Class>>();

        CSVM {
            num_total_sv,
            num_attributes,
            rho: Triangular::with_dimension(num_classes, Default::default()),
            kernel: Box::new(K::new_random()),
            probabilities: None,
            classes,
        }
    }
}

impl<'a, 'b> TryFrom<&'a str> for CSVM {
    type Error = SVMError;

    fn try_from(input: &'a str) -> Result<CSVM, SVMError> {
        let raw_model = ModelFile::try_from(input)?;

        // To quickly check what broke again during parsing ...
        // println!("{:?}", raw_model);

        let header = &raw_model.header;
        let vectors = &raw_model.vectors;

        // Get basic info
        let num_attributes = vectors[0].features.len();
        let num_classes = header.nr_class as usize;
        let num_total_sv = header.total_sv as usize;

        // Construct vector of classes
        let classes = (0..num_classes)
            .map(|class| {
                let label = header.label[class];
                let num_sv = header.nr_sv[class] as usize;
                Class::with_parameters(num_classes, num_sv, num_attributes, label)
            }).collect::<Vec<Class>>();

        let probabilities = match (&raw_model.header.prob_a, &raw_model.header.prob_b) {
            (&Some(ref a), &Some(ref b)) => Some(Probabilities {
                a: Triangular::from(a),
                b: Triangular::from(b),
            }),

            (_, _) => None,
        };

        let kernel: Box<dyn Kernel> = match raw_model.header.kernel_type {
            "rbf" => Box::new(Rbf::try_from(&raw_model)?),
            "linear" => Box::new(Linear::from(&raw_model)),
            _ => unimplemented!(),
        };

        // Allocate model
        let mut svm = CSVM {
            num_total_sv,
            num_attributes,
            probabilities,
            kernel,
            rho: Triangular::from(&header.rho),
            classes,
        };

        // Things down here are a bit ugly as the file format is a bit ugly ...

        // Now read all vectors and decode stored information
        let mut start_offset = 0;

        // In the raw file, support vectors are grouped by class
        for i in 0..num_classes {
            let num_sv_per_class = &header.nr_sv[i];
            let stop_offset = start_offset + *num_sv_per_class as usize;

            // Set support vector and coefficients
            for (i_vector, vector) in vectors[start_offset..stop_offset].iter().enumerate() {
                let mut last_attribute = None;

                // Set support vectors
                for (i_attribute, attribute) in vector.features.iter().enumerate() {
                    if let Some(last) = last_attribute {
                        // In case we have seen an attribute already, this one must be strictly
                        // the successor attribute
                        if attribute.index != last + 1 {
                            return Result::Err(SVMError::AttributesUnordered {
                                index: attribute.index,
                                value: attribute.value,
                                last_index: last,
                            });
                        }
                    };

                    let mut support_vectors = svm.classes[i].support_vectors.flat_mut();
                    support_vectors[(i_vector, i_attribute)] = attribute.value;

                    last_attribute = Some(attribute.index);
                }

                // Set coefficients
                for (i_coefficient, coefficient) in vector.coefs.iter().enumerate() {
                    let mut coefficients = svm.classes[i].coefficients.flat_mut();
                    coefficients[(i_coefficient, i_vector)] = f64::from(*coefficient);
                }
            }

            // Update last offset.
            start_offset = stop_offset;
        }

        // Return what we have
        Result::Ok(svm)
    }
}
