use ark_std::{end_timer, start_timer};
use halo2_proofs::{
    halo2curves::{
        bn256::{Bn256, Fr as Fp},
        ff::PrimeField,
        serde::SerdeObject,
    },
    poly::{
        commitment::{Blind, Params, ParamsProver},
        kzg::commitment::ParamsKZG,
        EvaluationDomain,
    },
};
use rand_core::{OsRng, RngCore};
use std::thread;

// #[feature(generic_const_exprs)]
use summa_solvency::merkle_sum_tree::utils::{
    build_merkle_tree_from_entries, parse_csv_to_entries,
};

const N_ASSETS: usize = 2;

fn main() {
    // Comparing MST commit vs KZG commit
    const K: u32 = 21;

    let entries =
        parse_csv_to_entries::<&str, N_ASSETS, 8>(&format!("./src/two_assets_entry_2_{}.csv", K))
            .unwrap();
    let depth = (entries.len() as f64).log2().ceil() as usize;

    let mst_commit_time = start_timer!(|| "Generate commit from Merkle Sum Tree");
    let mut nodes = vec![];
    let _root = build_merkle_tree_from_entries(&entries, depth, &mut nodes).unwrap();
    end_timer!(mst_commit_time);
    // println!("MST root: {:?}", _root);

    let params = ParamsKZG::<Bn256>::new(K + N_ASSETS as u32);

    // `j` is a degree of quotient polynomial, if we handling 4 vanishing polynomial `z` for user data,
    // hash of userdata, username and array of balance
    // j = K - 4, where K is a degree of polynomial `P(x)`
    let domain = EvaluationDomain::new(K + N_ASSETS as u32 - 4, K + N_ASSETS as u32);

    let kzg_commit_time = start_timer!(|| "Generate commit from Vector Polynomial");
    let mut handles = vec![];
    let chunk_size = (entries.len() + num_cpus::get() - 1) / num_cpus::get();
    for chunk in entries.chunks(chunk_size).map(|c| c.to_vec()) {
        // Convert each chunk to a Vec
        let handle = thread::spawn(move || {
            let mut local_result = vec![];
            for entry in chunk {
                local_result.push(Fp::from(entry.compute_leaf().hash));
                local_result
                    .push(Fp::from_raw_bytes(entry.username().as_bytes()).unwrap_or(Fp::zero()));
                for balance in entry.balances().iter() {
                    local_result.push(Fp::from_str_vartime(&balance.to_str_radix(10)[..]).unwrap());
                }
            }
            local_result
        });
        handles.push(handle);
    }

    let mut result = vec![];
    for handle in handles {
        result.extend(handle.join().unwrap());
    }
    // println!("polynomial coeff counts: {:?}", result.len());

    let a = domain.lagrange_from_vec(result);

    let alpha = Blind(Fp::from(OsRng.next_u64()));

    let _commit = params.commit_lagrange(&a, alpha);
    end_timer!(kzg_commit_time);
    // println!("KZG commit: {:?}", _commit);
}
