use ff::PrimeField;
use halo2_base::{
    halo2_proofs::halo2curves::{
        bn256::{Fr, G1Affine, G1, G2},
        FieldExt,
    },
    utils::ScalarField,
};
use halo2_ecc::commitments::utils::polynomial::Polynomial;
use serde::{Deserialize, Serialize};
use std::thread;

use summa_solvency::merkle_sum_tree::Entry;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub struct pp {
    pub ptau_g1: Vec<G1>,
    pub ptau_g2: Vec<G2>,
}

#[derive(Debug)]
pub struct UserData<const N_ASSETS: usize> {
    pub k: u32,
    pub pp: pp,
    pub entries: Vec<Entry<N_ASSETS>>,
    pub data: Vec<Fr>, // flatten data from entry, [H(username, balance_0, balance_1 ), balance_0, balance_1]
    pub p: Polynomial<Fr>,
}

pub fn root_of_unity(k: u32) -> Fr {
    Fr::root_of_unity().pow(&[2u64.pow(Fr::S - k) as u64, 0, 0, 0])
}

impl<const N_ASSETS: usize> UserData<N_ASSETS>
where
    [usize; N_ASSETS + 1]: Sized,
{
    /*
     * Returns ω - the generator for the roots of unity of order 2^k
     */
    pub fn root_of_unity(&self) -> Fr {
        root_of_unity(self.k)
    }

    /*
     * Instantiates UserData struct w/ public parameters, userdata data, and
     * polynomial p(X) that interpolates the userdata data.
     */
    pub fn new(entries: Vec<Entry<N_ASSETS>>, pp: pp, k: u32) -> Self {
        let w = root_of_unity(k);
        let mut idxs = vec![Fr::one()];
        for _ in 1..2u32.pow(k) as usize {
            idxs.push(idxs.last().unwrap() * w);
        }

        let mut data: Vec<Fr> = vec![Fr::zero()];

        // applied multi-threading to compute leaf hash
        let mut handles = vec![];
        let chunk_size = (entries.len() + num_cpus::get() - 1) / num_cpus::get();
        for chunk in entries.chunks(chunk_size) {
            let chunk = chunk.to_vec();
            handles.push(thread::spawn(move || {
                chunk
                    .into_iter()
                    .map(|entry| {
                        let mut v = vec![
                            Fr::from_bytes(&entry.compute_leaf().hash.to_bytes()).unwrap(),
                            Fr::from_bytes_le(&entry.username_to_big_uint().to_bytes_le()),
                        ];
                        entry
                            .balances()
                            .iter()
                            .for_each(|b| v.push(Fr::from_bytes_le(&b.to_bytes_le())));
                        v
                    })
                    .collect::<Vec<_>>()
            }));
        }

        for handle in handles {
            let result = handle.join().unwrap();
            for leaf_hash in result {
                data.extend(leaf_hash);
            }
        }

        // Pad data with zeros to be a power of 2
        if data.len() < 2u32.pow(k) as usize {
            for _ in data.len()..2u32.pow(k) as usize {
                data.push(Fr::zero());
            }
        }

        let p = Polynomial::from_points_ifft(data.clone(), w, k);
        UserData {
            k,
            pp,
            entries,
            data,
            p,
        }
    }

    /*
     * Convenience function for running a mock setup() for the commitment
     * scheme. This is not secure.
     */
    pub fn mock_trusted_setup(tau: Fr, userdata_len: usize, n_openings: usize) -> pp {
        let tau_fr: Fr = Fr::from(tau);

        // Powers of tau in G1 to commit to polynomials p(X) and q(X)
        let mut ptau_g1: Vec<G1> = vec![G1::generator()];
        for _ in 1..userdata_len {
            ptau_g1.push(ptau_g1.last().unwrap() * tau_fr);
        }

        // Powers of tau in G2 to commit to polynomials z(X) and r(X)
        let mut ptau_g2: Vec<G2> = vec![G2::generator()];
        for _ in 1..=n_openings {
            ptau_g2.push(ptau_g2.last().unwrap() * tau_fr);
        }

        pp { ptau_g1, ptau_g2 }
    }

    /*
     * Creates vector commitment by interpolating a polynomial p(X) and evaluating
     * at p(τ).
     */
    pub fn commit_vector(&self) -> G1Affine {
        G1Affine::from(self.p.eval_ptau(&self.pp.ptau_g1))
    }

    /*
     * Computes multi-open proof. Done by computing a quotient polynomial
     * q(X) = [p(X) - r(X)]/z(X). Opening proof is q(τ).
     */
    pub fn open_prf(&self, from: usize, to: usize) -> G1Affine {
        let selected_root = self.root_of_unity();

        let idxs = (from..to).collect::<Vec<usize>>();
        println!("idxs: {:?}", idxs);

        let idxs_fr: Vec<Fr> = idxs
            .iter()
            .map(|idx| selected_root.pow(&[*idx as u64, 0, 0, 0]))
            .collect();

        // let mut data: Vec<Fr> = vec![];
        // // TODO: refactor this logic
        // let node = self.entry[idx as usize].compute_leaf();
        // data.push(Fr::from_bytes_le(&node.hash.to_bytes()));
        // node.balances.iter().for_each(|b| data.push(Fr::from_bytes_le(&b.to_bytes())));

        let vals: Vec<Fr> = idxs.iter().map(|idx| self.data[*idx as usize]).collect();

        let r: Polynomial<Fr> = Polynomial::from_points_lagrange(&idxs_fr, &vals);
        let z: Polynomial<Fr> = Polynomial::vanishing(&idxs_fr);

        let (q, rem) = Polynomial::div_euclid(&(self.p.clone() - r.clone()), &z);
        println!("evaluated q and rem");
        if !rem.is_zero() {
            panic!("p(X) - r(X) is not divisible by z(X). Cannot compute q(X)");
        }

        G1Affine::from(q.eval_ptau(&self.pp.ptau_g1))
    }
}
