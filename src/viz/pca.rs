//! PCA-based dimensionality reduction for visualization

use crate::model::Thought;
use crate::viz::{VizExport, VizMeta, VizThought};
use crate::Result;

use linfa::traits::{Fit, Transformer};
use linfa::DatasetBase;
use linfa_reduction::Pca;
use ndarray::{Array1, Array2, Axis};

/// Project a collection of thoughts with embeddings into 3D space using PCA
pub fn project_to_3d(thoughts: &[Thought]) -> Result<VizExport> {
    // Separate thoughts with and without embeddings
    let (embedded, non_embedded): (Vec<_>, Vec<_>) =
        thoughts.iter().partition(|t| t.embedding.is_some());

    if embedded.is_empty() {
        // No embeddings, return all thoughts at origin
        let viz_thoughts: Vec<VizThought> = thoughts
            .iter()
            .map(|t| VizThought {
                id: t.id.0.clone(),
                content: t.content.clone(),
                thought_type: t.thought_type.clone(),
                position: [0.0, 0.0, 0.0],
                has_embedding: false,
                created_at: t.created_at,
            })
            .collect();

        return Ok(VizExport {
            thoughts: viz_thoughts,
            commits: vec![],
            meta: VizMeta {
                total_thoughts: thoughts.len(),
                embedded_thoughts: 0,
                reduction_method: "none".to_string(),
                original_dim: 0,
                variance_explained: None,
            },
        });
    }

    // Get the embedding dimension from the first embedded thought
    let dim = embedded[0].embedding.as_ref().unwrap().len();

    // PCA needs more samples than target dimensions for meaningful projection
    // With fewer samples, we use a simpler approach
    if embedded.len() < 4 {
        // Too few samples for meaningful PCA - distribute them in a simple pattern
        let mut viz_thoughts: Vec<VizThought> = Vec::with_capacity(thoughts.len());

        for (i, thought) in embedded.iter().enumerate() {
            // Spread points in a simple pattern, normalized to [0, 1]
            let angle = (i as f32) * 2.0 * std::f32::consts::PI / (embedded.len() as f32);
            let position = [
                0.5 + angle.cos() * 0.25, // Center at 0.5, radius 0.25
                0.5 + angle.sin() * 0.25,
                0.5,
            ];

            viz_thoughts.push(VizThought {
                id: thought.id.0.clone(),
                content: thought.content.clone(),
                thought_type: thought.thought_type.clone(),
                position,
                has_embedding: true,
                created_at: thought.created_at,
            });
        }

        for thought in &non_embedded {
            viz_thoughts.push(VizThought {
                id: thought.id.0.clone(),
                content: thought.content.clone(),
                thought_type: thought.thought_type.clone(),
                position: [0.5, 0.5, 0.5],
                has_embedding: false,
                created_at: thought.created_at,
            });
        }

        return Ok(VizExport {
            thoughts: viz_thoughts,
            commits: vec![],
            meta: VizMeta {
                total_thoughts: thoughts.len(),
                embedded_thoughts: embedded.len(),
                reduction_method: "simple".to_string(),
                original_dim: dim,
                variance_explained: None,
            },
        });
    }

    // Build the matrix of embeddings
    let n_samples = embedded.len();
    let mut data = Vec::with_capacity(n_samples * dim);

    for thought in &embedded {
        let emb = thought.embedding.as_ref().unwrap();
        // Ensure all embeddings have the same dimension
        if emb.len() != dim {
            return Err(crate::Error::Embedding(format!(
                "Inconsistent embedding dimensions: expected {}, got {}",
                dim,
                emb.len()
            )));
        }
        data.extend(emb.iter().map(|&x| x as f64));
    }

    let matrix = Array2::from_shape_vec((n_samples, dim), data)
        .map_err(|e| crate::Error::Embedding(format!("Failed to create matrix: {}", e)))?;

    // Create a dataset with dummy targets (PCA doesn't use them)
    let targets: Array1<()> = Array1::from_elem(n_samples, ());
    let dataset = DatasetBase::new(matrix, targets);

    // Perform PCA to reduce to 3 dimensions
    let pca = Pca::params(3)
        .fit(&dataset)
        .map_err(|e| crate::Error::Embedding(format!("PCA failed: {}", e)))?;

    // Get explained variance
    let variance = pca.explained_variance();
    let total_variance: f64 = variance.iter().sum();
    let variance_explained = if variance.len() >= 3 {
        Some([
            variance[0] / total_variance,
            variance[1] / total_variance,
            variance[2] / total_variance,
        ])
    } else {
        None
    };

    // Transform the data
    let projected = pca.transform(dataset);
    let coords = projected.records();

    // Get actual number of components (may be less than 3 if data is low-rank)
    let actual_dims = coords.ncols();

    // Normalize coordinates to roughly [-1, 1] range for the renderer
    let mut min_vals = [f64::MAX; 3];
    let mut max_vals = [f64::MIN; 3];

    for row in coords.axis_iter(Axis(0)) {
        for i in 0..actual_dims {
            min_vals[i] = min_vals[i].min(row[i]);
            max_vals[i] = max_vals[i].max(row[i]);
        }
        // Set defaults for missing dimensions
        for i in actual_dims..3 {
            min_vals[i] = 0.0;
            max_vals[i] = 1.0;
        }
    }

    let ranges: Vec<f64> = (0..3)
        .map(|i| {
            let range = max_vals[i] - min_vals[i];
            if range < 1e-10 {
                1.0
            } else {
                range
            }
        })
        .collect();

    // Build the visualization output
    let mut viz_thoughts: Vec<VizThought> = Vec::with_capacity(thoughts.len());

    // Add embedded thoughts with their 3D positions (normalized to [0, 1])
    for (i, thought) in embedded.iter().enumerate() {
        let row = coords.row(i);
        let position = [
            if actual_dims > 0 {
                ((row[0] - min_vals[0]) / ranges[0]) as f32
            } else {
                0.5
            },
            if actual_dims > 1 {
                ((row[1] - min_vals[1]) / ranges[1]) as f32
            } else {
                0.5
            },
            if actual_dims > 2 {
                ((row[2] - min_vals[2]) / ranges[2]) as f32
            } else {
                0.5
            },
        ];

        viz_thoughts.push(VizThought {
            id: thought.id.0.clone(),
            content: thought.content.clone(),
            thought_type: thought.thought_type.clone(),
            position,
            has_embedding: true,
            created_at: thought.created_at,
        });
    }

    // Add non-embedded thoughts at center
    for thought in &non_embedded {
        viz_thoughts.push(VizThought {
            id: thought.id.0.clone(),
            content: thought.content.clone(),
            thought_type: thought.thought_type.clone(),
            position: [0.5, 0.5, 0.5],
            has_embedding: false,
            created_at: thought.created_at,
        });
    }

    Ok(VizExport {
        thoughts: viz_thoughts,
        commits: vec![],
        meta: VizMeta {
            total_thoughts: thoughts.len(),
            embedded_thoughts: embedded.len(),
            reduction_method: "pca".to_string(),
            original_dim: dim,
            variance_explained,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Thought;

    #[test]
    fn test_project_empty() {
        let thoughts: Vec<Thought> = vec![];
        let result = project_to_3d(&thoughts).unwrap();
        assert_eq!(result.thoughts.len(), 0);
        assert_eq!(result.meta.embedded_thoughts, 0);
    }

    #[test]
    fn test_project_no_embeddings() {
        let thoughts = vec![Thought::new("Hello"), Thought::new("World")];
        let result = project_to_3d(&thoughts).unwrap();
        assert_eq!(result.thoughts.len(), 2);
        assert_eq!(result.meta.embedded_thoughts, 0);
        assert_eq!(result.meta.reduction_method, "none");
    }

    #[test]
    fn test_project_with_embeddings() {
        // Create thoughts with embeddings that have variance in multiple dimensions
        let mut thoughts = vec![];
        for i in 0..10 {
            let mut t = Thought::new(format!("Thought {}", i));
            // Create embeddings with variation in multiple dimensions
            // Using sin/cos to create non-linear spread across dimensions
            let emb: Vec<f32> = (0..10)
                .map(|j| {
                    let base = (i as f32 * 0.3 + j as f32 * 0.1).sin();
                    let offset = (j as f32 * 0.5).cos() * (i as f32 / 10.0);
                    base + offset
                })
                .collect();
            t.embedding = Some(emb);
            thoughts.push(t);
        }

        let result = project_to_3d(&thoughts).unwrap();
        assert_eq!(result.thoughts.len(), 10);
        assert_eq!(result.meta.embedded_thoughts, 10);
        assert_eq!(result.meta.reduction_method, "pca");
        assert_eq!(result.meta.original_dim, 10);
        // variance_explained may be None if data is low-rank (fewer than 3 principal components)
        // This is valid behavior for data with limited dimensionality

        // Check that positions are in [0, 1] range (normalized)
        for t in &result.thoughts {
            for &coord in &t.position {
                assert!(
                    coord >= 0.0 && coord <= 1.0,
                    "Coord {} out of range [0, 1]",
                    coord
                );
            }
        }
    }
}
