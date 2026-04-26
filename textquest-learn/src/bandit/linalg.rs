//! Minimal flat-array matrix arithmetic for the bandit algorithms.
//!
//! All matrices are stored as `Vec<f32>` in row-major order with shape `(d, d)`.
//! No external linear-algebra crate is required, keeping the DLL dependency graph small.

/// Return the identity matrix of size `d × d`.
pub fn identity(d: usize) -> Vec<f32> {
    let mut m = vec![0.0f32; d * d];
    for i in 0..d {
        m[i * d + i] = 1.0;
    }
    m
}

/// Matrix-vector product: result = m (d×d) @ v (d).
pub fn mat_vec(m: &[f32], v: &[f32], d: usize) -> Vec<f32> {
    debug_assert_eq!(m.len(), d * d);
    debug_assert_eq!(v.len(), d);
    let mut out = vec![0.0f32; d];
    for i in 0..d {
        let mut acc = 0.0f32;
        for j in 0..d {
            acc += m[i * d + j] * v[j];
        }
        out[i] = acc;
    }
    out
}

/// Dot product of two length-`d` slices.
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Scalar-multiply a vector in place.
pub fn scale_vec(v: &mut [f32], s: f32) {
    for x in v.iter_mut() {
        *x *= s;
    }
}

/// `a += b * scale` for vectors.
pub fn axpy(a: &mut [f32], b: &[f32], scale: f32) {
    debug_assert_eq!(a.len(), b.len());
    for (x, y) in a.iter_mut().zip(b.iter()) {
        *x += y * scale;
    }
}

/// Rank-1 Sherman-Morrison update of the inverse:
/// `a_inv := a_inv - (a_inv @ u @ v.T @ a_inv) / (1 + v.T @ a_inv @ u)`
///
/// For LinUCB arm update where `A += u u.T`, pass `u == v == x`.
pub fn sherman_morrison_update(a_inv: &mut [f32], u: &[f32], v: &[f32], d: usize) {
    debug_assert_eq!(a_inv.len(), d * d);
    debug_assert_eq!(u.len(), d);
    debug_assert_eq!(v.len(), d);

    // tmp1 = A_inv @ u
    let tmp1 = mat_vec(a_inv, u, d);
    // tmp2 = A_inv.T @ v  (= A_inv @ v when A_inv is symmetric)
    let tmp2 = mat_vec(a_inv, v, d);

    // denom = 1 + v.T @ tmp1
    let denom = 1.0 + dot(v, &tmp1);
    if denom.abs() < 1e-10 {
        return; // numerically singular; skip update
    }
    let inv_denom = 1.0 / denom;

    // a_inv -= outer(tmp1, tmp2) / denom
    for i in 0..d {
        for j in 0..d {
            a_inv[i * d + j] -= tmp1[i] * tmp2[j] * inv_denom;
        }
    }
}

/// Compute the lower-triangular Cholesky factor L such that M ≈ L L.T.
/// Returns `None` if M is not positive-definite (e.g., has not been updated yet).
/// `d` is the matrix side length; `m` is row-major `d × d`.
pub fn cholesky_lower(m: &[f32], d: usize) -> Option<Vec<f32>> {
    debug_assert_eq!(m.len(), d * d);
    let mut l = vec![0.0f32; d * d];
    for i in 0..d {
        for j in 0..=i {
            let mut s = m[i * d + j];
            for k in 0..j {
                s -= l[i * d + k] * l[j * d + k];
            }
            if i == j {
                if s <= 0.0 {
                    return None;
                }
                l[i * d + j] = s.sqrt();
            } else {
                l[i * d + j] = s / l[j * d + j];
            }
        }
    }
    Some(l)
}

/// Multiply lower-triangular L (d×d) by a vector z: result = L @ z.
pub fn lower_tri_mat_vec(l: &[f32], z: &[f32], d: usize) -> Vec<f32> {
    debug_assert_eq!(l.len(), d * d);
    debug_assert_eq!(z.len(), d);
    let mut out = vec![0.0f32; d];
    for i in 0..d {
        let mut acc = 0.0f32;
        for j in 0..=i {
            acc += l[i * d + j] * z[j];
        }
        out[i] = acc;
    }
    out
}

/// Compute `x.T A_inv x` where A_inv is a `d×d` matrix and x is length-`d`.
pub fn quadratic_form(a_inv: &[f32], x: &[f32], d: usize) -> f32 {
    let tmp = mat_vec(a_inv, x, d);
    dot(x, &tmp)
}

/// Scale a `d×d` matrix by scalar `s`.
pub fn mat_scale(m: &[f32], s: f32, d: usize) -> Vec<f32> {
    debug_assert_eq!(m.len(), d * d);
    m.iter().map(|v| v * s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_correct() {
        let id = identity(3);
        assert_eq!(id, vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn mat_vec_identity() {
        let id = identity(3);
        let v = vec![1.0f32, 2.0, 3.0];
        let r = mat_vec(&id, &v, 3);
        assert_eq!(r, v);
    }

    #[test]
    fn cholesky_known() {
        // 2x2: [[4, 2], [2, 3]] → L = [[2,0],[1, sqrt(2)]]
        let m = vec![4.0f32, 2.0, 2.0, 3.0];
        let l = cholesky_lower(&m, 2).unwrap();
        assert!((l[0] - 2.0).abs() < 1e-5);
        assert!((l[2] - 1.0).abs() < 1e-5);
        assert!((l[3] - 2.0f32.sqrt()).abs() < 1e-5);
    }

    #[test]
    fn sherman_morrison_agrees_with_direct() {
        // Start with I, apply SM for x=[1,0] → should match inverse of [[2,0],[0,1]]
        let d = 2;
        let mut a_inv = identity(d);
        let x = vec![1.0f32, 0.0];
        sherman_morrison_update(&mut a_inv, &x, &x, d);
        // new A_inv = (I + x x.T)^{-1} = [[0.5, 0],[0, 1]]
        assert!((a_inv[0] - 0.5).abs() < 1e-5, "a_inv[0] = {}", a_inv[0]);
        assert!((a_inv[3] - 1.0).abs() < 1e-5, "a_inv[3] = {}", a_inv[3]);
    }
}
